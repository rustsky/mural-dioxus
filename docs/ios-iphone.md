# Running Mural on your iPhone with Xcode

Mural isn't on the App Store, so iOS runs it only when it is signed for your own Apple ID and your
own phone. Xcode gives you what that takes, a development certificate and a provisioning profile,
and `scripts/install-ios-device.sh` builds, signs and installs the app with them. A free Apple ID is
enough.

The first time takes about 20 minutes, most of it downloads. After that, installing is one command.

## What you need

- A Mac with [Xcode](https://apps.apple.com/app/xcode/id497799835) (Mural is built and tested with
  Xcode 27 and the iOS 27 SDK).
- An iPhone and a USB cable.
- An Apple ID. A free one works; see [Free or paid Apple ID](#free-or-paid-apple-id).
- Rust with the iOS targets, and the Dioxus CLI matching the `dioxus` version in `Cargo.lock`:

  ```
  rustup target add aarch64-apple-ios aarch64-apple-ios-sim
  cargo install dioxus-cli --version 0.7.10 --locked
  ```

## 1. Set up Xcode (once)

1. Open Xcode once and let it install its components, or run:

   ```
   sudo xcodebuild -license accept
   xcodebuild -runFirstLaunch
   ```

2. Install the iOS platform if Xcode didn't: **Xcode → Settings → Components**, or
   `xcodebuild -downloadPlatform iOS` (about 8 GB; it includes the Simulator).

## 2. Add your Apple ID to Xcode (once)

1. **Xcode → Settings → Accounts**, press **+**, choose **Apple ID** and sign in.
2. Xcode creates a team called *Your Name (Personal Team)*. That's all a free account needs.

## 3. Prepare the iPhone (once)

1. Connect the iPhone with the cable and unlock it. When it asks **Trust This Computer?**, tap
   **Trust** and enter the passcode.
2. Turn on **Settings → Privacy & Security → Developer Mode** (the switch appears after the phone
   has been connected to Xcode once; if it's missing, do step 4 first and come back). The phone
   restarts and asks you to confirm.

## 4. Get a provisioning profile (once, and again if it expires)

The profile ties your certificate, your phone and one bundle identifier together. Xcode only makes
one for a project it builds, so make a throwaway project:

1. **File → New → Project…**, choose **iOS → App**, press **Next**.
2. Fill in:
   - **Product Name:** `mural`
   - **Team:** your Personal Team
   - **Organization Identifier:** something of your own, such as `com.yourname`. Together with the
     product name it forms the bundle identifier, here `com.yourname.mural`, which Mural will use.
3. Save the project anywhere.
4. At the top of the window, choose your iPhone as the run destination and press **Run** (▶).
   Xcode registers the phone, creates your *Apple Development* certificate and the profile, and
   installs an empty app.
5. If the phone refuses to open it, trust yourself on the phone:
   **Settings → General → VPN & Device Management**, tap your Apple ID, then **Trust**.

You can delete the empty app and the project now; the certificate and profile stay in Xcode.

## 5. Install Mural

With the iPhone connected and unlocked, from the `mural-desktop` folder:

```
./scripts/install-ios-device.sh
```

The script:

1. builds Mural for iPhone (`scripts/build-ios.sh`),
2. takes the newest provisioning profile Xcode downloaded and gives the app its bundle identifier,
3. signs the voice framework and the app with your Apple Development certificate,
4. installs the app on the connected iPhone and opens it.

To use a particular profile instead of the newest one, pass its path:

```
./scripts/install-ios-device.sh path/to/profile.mobileprovision
```

Xcode keeps profiles in `~/Library/Developer/Xcode/UserData/Provisioning Profiles/`.

On first launch, allow the microphone when asked, and add your API key in Mural's Settings to hold
a conversation. The on-device voice for Shadow downloads the first time you open a drill (about
500 MB per language).

## Keeping it working

### Every 7 days (free Apple ID)

Apple lets apps signed with a free Apple ID run for 7 days. After that Mural stops opening. Connect
the phone and run `./scripts/install-ios-device.sh` again; your conversations and learning stay on
the phone.

If the script says the profile has expired, repeat step 4 (open the throwaway project, or a new
one with the same bundle identifier, and press **Run**) to get a fresh one.

### Developer Mode

Keep Developer Mode on while you use Mural this way: with it off, iOS refuses to open apps signed
with a development certificate (Mural stays installed and opens again once you turn it back on).

Developer Mode doesn't weaken the phone's protection against the internet. It lets apps signed with
a developer certificate run, and lets a computer you have trusted debug apps. Install only apps you
built yourself or trust, don't trust other people's computers, and turn Developer Mode off when you
stop using Mural or before you lend or sell the phone.

### Free or paid Apple ID

| | Free Apple ID | Apple Developer Program ($99/year) |
| --- | --- | --- |
| Signature lasts | 7 days | 1 year |
| Apps signed this way per phone | 3 | No limit |
| Install without a cable | No | Yes, through TestFlight |

## Trying it in the Simulator first

```
./scripts/build-ios.sh --sim
xcrun simctl boot "iPhone 18 Pro"
open -a Simulator
xcrun simctl install booted target/ios/release/dx/mural/release/ios/Mural.app
xcrun simctl launch booted chat.mural.desktop
```

The Simulator has no microphone, so speaking in Talk and Shadow can only be tried on a phone.

## Without Xcode

`./scripts/build-ios.sh` (no arguments) makes an unsigned `dist/Mural-<version>-ios-unsigned.ipa`.
[Sideloadly](https://sideloadly.io) or [AltStore](https://altstore.io) can sign it with your Apple
ID and install it; see *Installing on an iPhone* in the README. The 7-day limit and Developer Mode
apply the same way.

## Troubleshooting

| Message or symptom | What to do |
| --- | --- |
| `No provisioning profile found` | Do step 4. |
| `No Apple Development certificate` | Add your Apple ID in Xcode (step 2), then do step 4. |
| `No iPhone connected (or it isn't trusted / in Developer Mode)` | Unlock the phone, check the cable, tap **Trust**, and turn on Developer Mode (step 3). `xcrun devicectl list devices` shows what Xcode sees. |
| Installs but won't open: *Untrusted Developer* | **Settings → General → VPN & Device Management**, tap your Apple ID, **Trust**. |
| Opened for a week, now won't | The free signature expired; run the install script again. |
| *Unable to install* / *The app's bundle identifier doesn't match the profile* | The profile belongs to another app; pass the profile you made in step 4 as an argument. |
| *Maximum number of apps for free development profiles reached* | A free Apple ID allows 3 such apps; delete one you made with Xcode or another tool. |
| `You have not agreed to the Xcode license agreements` | `sudo xcodebuild -license accept` |
| `dx` build errors about `SherpaOnnxC.framework` or the Cargo profile directory | Build through the scripts, which set the target folder sherpa-onnx needs. |
| Opens and closes at once | Rebuild with the current source: iOS 27 needs the scene-based app life cycle, which the vendored tao in `vendor/tao` provides. |
