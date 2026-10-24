# Statuses produced by real conditions, without fault injection and without
# administrator rights (docs/tests-build-livraison.md). Test files only, in
# a folder of their own, deleted at the end.
#
#   powershell -ExecutionPolicy Bypass -File tools\manual\real-cases.ps1 [-Folder <folder>]
#
# Needs the release CLI: cargo build --release -p fileseal-cli
# The network cases map the administrative share \\localhost\C$ to a free
# drive letter (the folder must be on C:), then disconnect it.

param(
    [string]$Folder = (Join-Path $env:TEMP 'fileseal-real-cases'),
    [string]$Cli = (Join-Path $PSScriptRoot '..\..\target\release\fileseal-cli.exe')
)
$ErrorActionPreference = 'Continue'
if (-not (Test-Path $Cli)) { throw "build the CLI first: cargo build --release -p fileseal-cli" }
Remove-Item -Recurse -Force $Folder -ErrorAction SilentlyContinue
$d = (New-Item -ItemType Directory -Force $Folder).FullName

function Run([string[]]$arguments) {
    # The CLI writes its selection summary and its errors on stderr: kept
    # as plain lines.
    return & $Cli @arguments 2>&1 | ForEach-Object { "$_" }
}
function Show($label, $output) {
    $line = $output | Select-String -Pattern '^#\d+\s' | Select-Object -First 1
    $detail = $output | Select-String -Pattern '^\t' | Select-Object -Last 1
    "{0,-30} {1}" -f $label, ($line -replace '\t', '  ')
    if ($detail) { "{0,-30}   {1}" -f '', $detail.Line.Trim() }
}
function Summary($label, $output) {
    "{0,-30} {1}" -f $label, (($output | Select-String -Pattern 'files:|stopped|volume lost|^\t(match|noReference):') -join ' / ')
}

New-Item -ItemType Directory -Force "$d\files", "$d\closed" | Out-Null
foreach ($n in 'plain', 'locked', 'denied', 'conflict', 'invalid', 'changed', 'gone') {
    Set-Content -Path "$d\files\$n.bin" -Value "content of $n" -Encoding ascii
}
Set-Content -Path "$d\closed\partial.bin" -Value 'content of partial' -Encoding ascii
$db = "$d\real.fileseal"
Run @('init-db', $db, '--name', 'Real cases') | Out-Null
Run @('create', "$d\files", '--db', $db) | Out-Null

Show 'match' (Run @('verify', "$d\files\plain.bin", '--db', $db))
Set-Content -Path "$d\files\changed.bin" -Value 'changed afterwards' -Encoding ascii
Show 'content changed' (Run @('verify', "$d\files\changed.bin", '--db', $db))
# Deleted after its hash file was written: the line of that hash file names
# a file that is no longer there.
Remove-Item "$d\files\gone.bin"
Show 'file deleted' (Run @('verify', "$d\files\gone.bin.sha256"))
Set-Content -Path "$d\new.bin" -Value 'never referenced' -Encoding ascii
Show 'never referenced' (Run @('verify', "$d\new.bin", '--db', $db))

$holder = [IO.File]::Open("$d\files\locked.bin", 'Open', 'ReadWrite', 'None')
Show 'open elsewhere for writing' (Run @('verify', "$d\files\locked.bin", '--db', $db))
$holder.Close()

icacls "$d\files\denied.bin" /deny '*S-1-1-0:(R)' | Out-Null
Show 'read denied' (Run @('verify', "$d\files\denied.bin", '--db', $db))
icacls "$d\files\denied.bin" /remove:d '*S-1-1-0' | Out-Null

Set-Content -Path "$d\files\conflict.bin" -Value 'changed content' -Encoding ascii
Remove-Item "$d\files\conflict.bin.sha256"
Run @('create', "$d\files\conflict.bin") | Out-Null
Show 'hash file vs database' (Run @('verify', "$d\files\conflict.bin", '--db', $db))

Set-Content -Path "$d\files\invalid.bin.sha256" -Value 'this is not a hash' -Encoding ascii
Show 'invalid hash file' (Run @('verify', "$d\files\invalid.bin"))

icacls "$d\closed" /deny '*S-1-1-0:(W,WD,AD)' | Out-Null
Show 'folder not writable' (Run @('create', "$d\closed\partial.bin", '--db', $db))
icacls "$d\closed" /remove:d '*S-1-1-0' | Out-Null

attrib +R $db
Show 'database read-only, verify' (Run @('verify', "$d\files\plain.bin", '--db', $db))
"{0,-30} {1}" -f 'database read-only, create', (Run @('create', "$d\files\plain.bin", '--db', $db) | Select-Object -Last 1)
attrib -R $db

# Creating again: the same content, a content changed since, and a file
# moved here under another name.
Show 'create again, same content' (Run @('create', "$d\files\plain.bin", '--db', $db))
Show 'create again, adding MD5' (Run @('create', "$d\files\plain.bin", '--db', $db, '--algo', 'sha256', '--algo', 'md5'))
Show 'create again, other content' (Run @('create', "$d\files\changed.bin", '--db', $db, '--no-sidecar'))
Move-Item "$d\files\locked.bin" "$d\moved.bin"
Remove-Item "$d\files\locked.bin.sha256"
Show 'create, file moved here' (Run @('create', "$d\moved.bin", '--db', $db, '--no-sidecar'))

# Searching the database by content.
Show 'search, known here' (Run @('search', "$d\files\plain.bin", '--db', $db))
Show 'search, moved here' (Run @('search', "$d\moved.bin", '--db', $db))
Copy-Item "$d\files\plain.bin" "$d\copy of plain.bin"
Show 'search, a copy' (Run @('search', "$d\copy of plain.bin", '--db', $db))
Show 'search, unknown' (Run @('search', "$d\new.bin", '--db', $db))
Show 'search, changed here' (Run @('search', "$d\files\changed.bin", '--db', $db))
# Two recorded copies of one content, both gone: a third one could be either.
New-Item -ItemType Directory -Force "$d\twins" | Out-Null
foreach ($n in 'a', 'b') { Set-Content -Path "$d\twins\$n.bin" -Value 'twins' -Encoding ascii }
Run @('create', "$d\twins", '--db', $db, '--no-sidecar') | Out-Null
Remove-Item -Recurse "$d\twins"
Set-Content -Path "$d\third.bin" -Value 'twins' -Encoding ascii
Show 'search, two origins' (Run @('search', "$d\third.bin", '--db', $db))

# A hard link: the same physical file twice in one list.
New-Item -ItemType HardLink -Path "$d\link to plain.bin" -Target "$d\files\plain.bin" | Out-Null
$twice = Run @('verify', "$d\files\plain.bin", "$d\link to plain.bin", '--db', $db)
"{0,-30} {1}" -f 'hard link', (($twice | Select-String -Pattern '^#2\s') -replace '\t', '  ')

# Marked offline (attrib +O), one of the attributes of cloud placeholders:
# skipped when its folder is listed, kept when chosen directly.
New-Item -ItemType Directory -Force "$d\cloud" | Out-Null
Set-Content -Path "$d\cloud\online only.txt" -Value 'not on this PC' -Encoding ascii
attrib +O "$d\cloud\online only.txt"
"{0,-30} {1}" -f 'offline file, its folder', (Run @('scan', "$d\cloud") | Select-String -Pattern '^excluded:')
"{0,-30} {1}" -f 'offline file, chosen', (Run @('scan', "$d\cloud\online only.txt") | Select-String -Pattern '^\d+ files')
attrib -O "$d\cloud\online only.txt"

$big = "$d\big.bin"
$fs = [IO.File]::Create($big); $fs.SetLength(3GB); $fs.Close()
$first = Start-Process -FilePath $Cli -ArgumentList 'create', "`"$big`"", '--db', "`"$db`"", '--no-sidecar' -PassThru -NoNewWindow -RedirectStandardOutput "$d\first.txt"
Start-Sleep -Milliseconds 400
"{0,-30} {1}" -f 'database used elsewhere', (Run @('verify', "$d\files\plain.bin", '--db', $db) | Select-Object -Last 1)
$first.WaitForExit(120000) | Out-Null
Remove-Item $big

# Network: the folder shared through \\localhost\C$, as a drive letter.
if ($d.Substring(0, 2) -ne 'C:') { "network cases skipped: the folder must be on C:"; return }
$letter = @('Z', 'Y', 'X', 'V', 'U', 'T') | Where-Object { -not (Test-Path "$($_):\") } | Select-Object -First 1
New-Item -ItemType Directory -Force "$d\net\Videos", "$d\net\Docs", "$d\many" | Out-Null
$chunk = New-Object byte[] (64MB); (New-Object Random 42).NextBytes($chunk)
foreach ($i in 1..4) { $f = [IO.File]::Create("$d\net\Videos\video $i.mkv"); foreach ($j in 1..16) { $f.Write($chunk, 0, $chunk.Length) }; $f.Close() }
foreach ($i in 1..30) { Set-Content -Path "$d\net\Docs\doc $i.txt" -Value "doc $i" -Encoding ascii }
foreach ($i in 1..2000) { [IO.File]::WriteAllText("$d\many\f$i.bin", "x$i") }
$unc = '\\localhost\C$' + $d.Substring(2) + '\net'
net use "$($letter):" $unc | Out-Null
"{0,-30} {1}" -f 'drive letter = its share', (Run @('scan', "$($letter):\Docs", "$unc\Docs") | Select-String -Pattern '^excluded:')
Run @('create', "$($letter):\Videos") | Out-Null

# The share is lost while a file is read.
$job = Start-Process -FilePath $Cli -ArgumentList 'verify', "$($letter):\Videos", "$($letter):\Docs", '--parallel', '1' -PassThru -NoNewWindow -RedirectStandardOutput "$d\share-lost.txt"
Start-Sleep -Milliseconds 700
net use "$($letter):" /delete /y | Out-Null
$job.WaitForExit(120000) | Out-Null
$lost = Get-Content "$d\share-lost.txt"
Show 'share lost: file being read' ($lost | Select-String -Pattern 'readError' -Context 0, 1 | ForEach-Object { $_.Line; $_.Context.PostContext })
Summary 'share lost: the batch' $lost

# The database is on the share, and the share is lost during a batch.
net use "$($letter):" $unc | Out-Null
Run @('init-db', "$($letter):\net.fileseal", '--name', 'On the network') | Out-Null
$job = Start-Process -FilePath $Cli -ArgumentList 'create', "`"$d\many`"", '--db', "$($letter):\net.fileseal", '--no-sidecar' -PassThru -NoNewWindow -RedirectStandardOutput "$d\db-lost.txt"
Start-Sleep -Milliseconds 500
net use "$($letter):" /delete /y | Out-Null
$job.WaitForExit(120000) | Out-Null
Summary 'database lost' (Get-Content "$d\db-lost.txt")
net use "$($letter):" $unc | Out-Null
Summary 'kept once reconnected' (Run @('verify', "$d\many", '--db', "$($letter):\net.fileseal", '--no-sidecar'))
net use "$($letter):" /delete /y | Out-Null
Remove-Item -Recurse -Force $d -ErrorAction SilentlyContinue
