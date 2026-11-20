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
  just test-kotlin

test-rust *FLAGS:
  STELLAR_BENCH_QUICK=1 cargo nextest run --all-targets {{FLAGS}}

test-kotlin *FLAGS:
  ./gradlew :shared:jvmTest --quiet {{FLAGS}}

# Render `@Preview` functions to PNGs in shared/build/previews/<source file>/. FILTER matches names by substring.
preview *FILTER:
  rm -rf shared/build/previews
  STELLAR_RENDER_PREVIEWS=1 STELLAR_RENDER_PREVIEWS_FILTER={{quote(FILTER)}} ./gradlew :shared:jvmTest --tests 'net.trillia.stellar.RenderPreviewsTest' --rerun --quiet
  find "$PWD/shared/build/previews" -name '*.png' | sort

run-tui *FLAGS:
  cargo run --package stellar-tui -- {{FLAGS}}

run-desktop *FLAGS:
  ./gradlew :desktopApp:run {{ if FLAGS != "" { "--args=" + quote(FLAGS) } else { "" } }}

run-desktop-hot *FLAGS:
  ./gradlew :desktopApp:hotRun --auto {{ if FLAGS != "" { "--args=" + quote(FLAGS) } else { "" } }}

run-desktop-release *FLAGS:
  ./gradlew :desktopApp:createDistributable -Pnet.trillia.stellar.rust.variant=release
  ./{{desktop-binary}} {{FLAGS}}

run-mobile *FLAGS:
  ./gradlew :desktopApp:run --args={{ quote("--mobile " + FLAGS) }}

run-mobile-hot *FLAGS:
  ./gradlew :desktopApp:hotRun --auto --args={{ quote("--mobile " + FLAGS) }}

run-android:
  ./gradlew :androidApp:installDebug
  adb shell am start -n net.trillia.stellar/.MainActivity
