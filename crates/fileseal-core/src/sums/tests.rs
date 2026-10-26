use super::*;

const A: &str = "5d7f13f63a3460bd0f82113728dd5431982147b9087d95540cfc9b892a110043";
const B: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
const C: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";

fn sha(hex: &str) -> Digest {
    Digest::parse(Algorithm::Sha256, hex).unwrap()
}

fn line(name: &str, hex: &str) -> (String, Digest) {
    (name.to_owned(), sha(hex))
}

fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn one_list_per_algorithm_named_as_gnu_does() {
    assert_eq!(name_for(Algorithm::Sha256), "SHA256SUMS");
    assert_eq!(name_for(Algorithm::Md5), "MD5SUMS");
    assert_eq!(
        path_for(Path::new(r"D:\Movies\Alien.mkv"), Algorithm::Md5).unwrap(),
        Path::new(r"D:\Movies\MD5SUMS")
    );
}

#[test]
fn a_new_list_is_written_sorted_in_gnu_format() {
    let dir = tempfile::tempdir().unwrap();
    let (path, outcomes) = add(
        dir.path(),
        Algorithm::Sha256,
        &[
            line("Blade Runner (1982).mkv", A),
            line("Alien (1979).mkv", B),
        ],
    )
    .unwrap();
    assert_eq!(outcomes, [Added::Written, Added::Written]);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        format!("{B}  Alien (1979).mkv\n{A}  Blade Runner (1982).mkv\n"),
        "UTF-8, LF, sorted by name"
    );
    assert_eq!(
        names_in(dir.path()),
        ["SHA256SUMS"],
        "no temporary file left"
    );
}

#[test]
fn lines_are_added_and_existing_ones_never_changed() {
    let dir = tempfile::tempdir().unwrap();
    let list = dir.path().join("SHA256SUMS");
    // Written by another tool: binary mark, a comment, no final newline.
    let before = format!("# made by hand\n{A} *Blade Runner (1982).mkv");
    std::fs::write(&list, &before).unwrap();
    let (_, outcomes) = add(
        dir.path(),
        Algorithm::Sha256,
        &[
            line("Blade Runner (1982).mkv", A),
            line("Alien (1979).mkv", B),
            line("Dune (2021).mkv", C),
        ],
    )
    .unwrap();
    assert_eq!(
        outcomes,
        [Added::AlreadyPresent, Added::Written, Added::Written]
    );
    assert_eq!(
        std::fs::read_to_string(&list).unwrap(),
        format!("{before}\n{B}  Alien (1979).mkv\n{C}  Dune (2021).mkv\n"),
        "the old lines byte for byte, then the new ones"
    );
}

#[test]
fn another_value_for_a_name_is_a_conflict_left_as_it_is() {
    let dir = tempfile::tempdir().unwrap();
    let list = dir.path().join("SHA256SUMS");
    let before = format!("{A}  Alien (1979).mkv\n");
    std::fs::write(&list, &before).unwrap();
    let (_, outcomes) = add(
        dir.path(),
        Algorithm::Sha256,
        &[line("Alien (1979).mkv", B)],
    )
    .unwrap();
    assert_eq!(
        outcomes,
        [Added::Conflict {
            existing: Some(A.to_owned())
        }]
    );
    assert_eq!(std::fs::read_to_string(&list).unwrap(), before, "untouched");
}

#[test]
fn a_list_in_crlf_stays_in_crlf() {
    let dir = tempfile::tempdir().unwrap();
    let list = dir.path().join("SHA256SUMS");
    std::fs::write(&list, format!("{A}  a.mkv\r\n")).unwrap();
    add(dir.path(), Algorithm::Sha256, &[line("b.mkv", B)]).unwrap();
    assert_eq!(
        std::fs::read_to_string(&list).unwrap(),
        format!("{A}  a.mkv\r\n{B}  b.mkv\r\n")
    );
}

#[test]
fn a_list_that_cannot_be_read_is_never_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let list = dir.path().join("SHA256SUMS");
    std::fs::write(&list, "this is not a hash list\n").unwrap();
    let refused = add(dir.path(), Algorithm::Sha256, &[line("a.mkv", A)]);
    assert_eq!(refused.unwrap_err().kind, ProblemKind::InvalidHashFile);
    assert_eq!(
        std::fs::read_to_string(&list).unwrap(),
        "this is not a hash list\n"
    );
}

#[test]
fn a_failed_publication_leaves_the_list_as_it_was() {
    let dir = tempfile::tempdir().unwrap();
    let list = dir.path().join("SHA256SUMS");
    std::fs::write(&list, format!("{A}  a.mkv\n")).unwrap();
    faults::arm(FaultPoint::SidecarPublish);
    let result = add(dir.path(), Algorithm::Sha256, &[line("b.mkv", B)]);
    faults::clear();
    assert!(result.is_err());
    assert_eq!(
        std::fs::read_to_string(&list).unwrap(),
        format!("{A}  a.mkv\n")
    );
    assert_eq!(
        names_in(dir.path()),
        ["SHA256SUMS"],
        "no temporary file left"
    );
}

#[test]
fn a_file_is_found_by_its_name_only() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("Alien (1979).mkv");
    assert_eq!(lookup(&file, Algorithm::Sha256), SidecarLookup::Absent);
    // A list's only line is about another file: not this one's.
    std::fs::write(
        dir.path().join("SHA256SUMS"),
        format!("{A}  Blade Runner (1982).mkv\n"),
    )
    .unwrap();
    assert_eq!(lookup(&file, Algorithm::Sha256), SidecarLookup::Absent);
    std::fs::write(
        dir.path().join("SHA256SUMS"),
        format!("{A}  Blade Runner (1982).mkv\n{B}  ./Alien (1979).mkv\n"),
    )
    .unwrap();
    match lookup(&file, Algorithm::Sha256) {
        SidecarLookup::Found { digest, notes, .. } => {
            assert_eq!(digest.hex, B);
            assert!(notes.is_empty());
        }
        other => panic!("expected the line, got {other:?}"),
    }
}

#[test]
fn case_is_ignored_where_the_file_system_ignores_it() {
    if !cfg!(windows) {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("SHA256SUMS"), format!("{A}  ALIEN.MKV\n")).unwrap();
    match lookup(&dir.path().join("Alien.mkv"), Algorithm::Sha256) {
        SidecarLookup::Found { notes, .. } => assert_eq!(
            notes,
            [SidecarNote::NameCaseDiffers {
                written: "ALIEN.MKV".to_owned()
            }]
        ),
        other => panic!("expected the line, got {other:?}"),
    }
}

#[test]
fn lines_that_disagree_or_of_another_algorithm_are_unusable() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.mkv");
    std::fs::write(
        dir.path().join("SHA256SUMS"),
        format!("{A}  a.mkv\n{B}  a.mkv\n"),
    )
    .unwrap();
    assert!(matches!(
        lookup(&file, Algorithm::Sha256),
        SidecarLookup::Invalid { .. }
    ));
    std::fs::write(
        dir.path().join("MD5SUMS"),
        format!("SHA256 (a.mkv) = {A}\n"),
    )
    .unwrap();
    assert!(matches!(
        lookup(&file, Algorithm::Md5),
        SidecarLookup::Invalid { .. }
    ));
}

#[test]
fn a_list_changed_on_the_disk_is_read_again() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.mkv");
    let list = dir.path().join("SHA256SUMS");
    std::fs::write(&list, format!("{A}  b.mkv\n")).unwrap();
    assert_eq!(lookup(&file, Algorithm::Sha256), SidecarLookup::Absent);
    std::fs::write(&list, format!("{A}  b.mkv\n{B}  a.mkv\n")).unwrap();
    assert!(matches!(
        lookup(&file, Algorithm::Sha256),
        SidecarLookup::Found { .. }
    ));
    // And after FileSeal's own writing.
    add(dir.path(), Algorithm::Sha256, &[line("c.mkv", C)]).unwrap();
    assert!(matches!(
        lookup(&dir.path().join("c.mkv"), Algorithm::Sha256),
        SidecarLookup::Found { .. }
    ));
}
