gil is an interactive version of git-log

## installing

assuming `~/bin` is on your `PATH`,
```sh
cd ~/bin
curl -L https://github.com/raylu/gil/releases/latest/download/gil-$(uname -m | sed s/arm64/aarch64/)-$(uname -s | awk '{print tolower($0)}' | sed -e s/darwin/apple-darwin/ -e s/linux/unknown-linux-gnu/) -o gil
chmod +x gil
```

alternatively, download a binary from the [releases](https://github.com/raylu/gil/releases) page

or install from source via `cargo install gil`: https://crates.io/crates/gil

### macOS

```sh
brew install --cask raylu/formulae/gil
```

if you downloaded manually and get an error about how it "can’t be opened because Apple cannot check it 
or malicious software", this is because the quarantine extended attribute has been set by your browser.
either `xattr -d com.apple.quarantine gil` or use `curl`/`wget` to download instead
