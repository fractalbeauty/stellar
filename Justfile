opener := if os() == "macos" {
  "open"
} else {
  "xdg-open"
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
  cargo nextest run {{FLAGS}}

run-tui *FLAGS:
  cargo run --package stellar-tui -- {{FLAGS}}

run-desktop:
  ./gradlew :desktopApp:run

run-desktop-hot:
  ./gradlew :desktopApp:hotRun --auto

run-desktop-release:
  ./gradlew :desktopApp:runDistributable -Pnet.trillia.stellar.rust.variant=release

run-android:
  ./gradlew :androidApp:installDebug
  adb shell am start -n net.trillia.stellar/.MainActivity
