@echo off
rem Toutes les verifications, dans l'ordre. S'arrete a la premiere qui echoue.
setlocal
cd /d "%~dp0.."
call npx prettier --check . || exit /b 1
call npm run lint || exit /b 1
call npm run check || exit /b 1
call npm run test || exit /b 1
cargo fmt --all --check || exit /b 1
cargo clippy --workspace --all-targets -- -D warnings || exit /b 1
rem cargo test regenere aussi les types TypeScript (ui\src\lib\generated).
cargo test --workspace || exit /b 1
git diff --quiet -- ui/src/lib/generated || (
  echo Les types TypeScript generes ont change ^(normal apres la modification d'un type Rust exporte^) : les verifier, puis les commiter.
  exit /b 1
)
echo Toutes les verifications sont passees.
