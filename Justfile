opener := if os() == "macos" {
  "open"
} else {
  "xdg-open"
}

desktop-binary := if os() == "macos" {
  "desktopApp/build/compose/binaries/main/app/net.trillia.stellar.app/Contents/MacOS/net.trillia.stellar"
} else if os() == "windows" {
  "desktopApp/build/compose/binaries/main/app/net.trillia.stellar.exe"
} else {
  "desktopApp/build/compose/binaries/main/app/net.trillia.stellar"
}

default:
  just --list

test:
  cargo check --workspace
  cargo fmt --check
  just test-rust
  ./gradlew compileKotlin --quiet
  ./gradlew ktlintFormat --quiet

test-rust *FLAGS:
  STELLAR_BENCH_QUICK=1 cargo nextest run --all-targets {{FLAGS}}

run-tui *FLAGS:
  cargo run --package stellar-tui -- {{FLAGS}}

run-desktop *FLAGS:
  ./gradlew :desktopApp:run --args="{{FLAGS}}"

run-desktop-hot *FLAGS:
  ./gradlew :desktopApp:hotRun --auto --args="{{FLAGS}}"

run-desktop-release *FLAGS:
  ./gradlew :desktopApp:createDistributable -Pnet.trillia.stellar.rust.variant=release
  ./{{desktop-binary}} {{FLAGS}}

run-android:
  ./gradlew :androidApp:installDebug
  adb shell am start -n net.trillia.stellar/.MainActivity
