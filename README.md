# Key Finder

I struggle with learning shortcuts, so I made an app that shows periodic notifications with keyboard shortcut reminders.

This project currently supports **macOS only**.

## Running locally

There are two ways to use this app:
- As a cli app (via terminal)
- As a macos tray app

A native app is way more friendly (and fully featured) compared to a terminal app, but it's easier and quicker to develop and test in the terminal mode because of all the macos permissions.

### Terminal Mode

Requires terminal-notifier to display notifications (`brew install terminal-notifier`)

```bash
cargo run
```
Run the app with `--help` to see available configuration options (or see [src/cli/options.rs](/Users/zelenya/projects/keys/src/cli/options.rs)).

```bash
cargo run -- --notify-interval 2minutes
```

You can also use matching environment variables:

```bash
NOTIFY_INTERVAL=2minutes cargo run
```

### Bundled / tray mode

Install bundler:

```bash
cargo install cargo-bundle
```

Build the app bundle:

```bash
cargo bundle --release
```

Bundle output: `target/release/bundle/osx/Key Finder.app`

Run and test it in foreground with:

```bash
"target/release/bundle/osx/Key Finder.app/Contents/MacOS/key-finder"
```

---

Note that executable by itself won't have enough permissions to send notification or interact with other windows.

---

You can give permissions to the terminal and do basic testing. 

But to access all the features, ad-hoc sign:

```bash
codesign --force --deep --sign - "target/release/bundle/osx/Key Finder.app"
```

1. Open `Key Finder.app` from Finder.
2. Wait for the first notification prompt and click **Allow**.
3. Verify in **System Settings > Notifications > Key Finder**.

If notification permission get into a bad state:

```bash
tccutil reset Notifications com.zelenya.keyfinder
```

Then relaunch the app and allow again.

### GitHub release

For a simple personal release, use the helper script:

```bash
./scripts/release.sh vX.Y.Z
```

It will:
- build the macOS `.app`, ad-hoc sign it, and zip it;
- create and push the git tag;
- create a GitHub release.

Requirements:
- `gh auth login`
- `cargo-bundle` installed (let the script install it for you)

### Runtime settings 

Shortcuts and settings are stored in sqlite. Default database path:

`~/Library/Application Support/Key Finder/library.db`

Settings precedence at startup:
- cli flags
- environment variables
- sqlite `settings` values
- built-in defaults

When you change settings in the tray UI, it saves them to sqlite and updates the running notification interval directly. It doesn't rewrite shell or process environment variables (those will be ignored now).

Inspect persisted settings:

```bash
sqlite3 "$HOME/Library/Application Support/Key Finder/library.db" \
  "select key, value, updated_at from settings order by key;"
```

## Import Shortcuts

On best effort basis.

### Custom csv import

For custom apps, you can import a simple two-column CSV file:

```csv
shortcut,description
cmd+shift+d,split pane down
cmd+d,split pane right
```

The header row is optional. The file just needs rows in `shortcut,description` format.

A sample fixture lives at [testdata/importers/ghostty-shortcuts.csv](/Users/zelenya/projects/keys/testdata/importers/ghostty-shortcuts.csv).

### Zed import
  1. In Zed, run `zed: open default keymap` (cmd + shift + p or something like that)
  2. Save it as a file 
  3. In Key Finder UI, open zed, click `Import`, and choose that json file
  4. Note that you can find your overrides in a separate json config (something like `~/.config/zed/keymap.json`)

### VS Code import
  1. In VS Code: run `Preferences: Open Default Keyboard Shortcuts (JSON)`
  2. Save it as a file
  3. In Key Finder UI, open VS Code, click `Import`, and choose that json or jsonc file
  4. Or switch the import mode to `Installed extension shortcuts` to scan installed VS Code extension manifests without choosing a file

### IntelliJ IDEA import
  1. Open your IntelliJ IDEA and export or locate the keymap XML you want to import
  2. In Key Finder, open the matching app, click `Import`, and choose that XML file
  3. If Key Finder finds a local IntelliJ IDEA keymap XML, it may prefill that path for you
