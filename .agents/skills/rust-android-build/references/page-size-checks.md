# 16 KiB package and device checks

Read this when you run the package check on an APK or AAB, or the device check on a 16 KB
emulator image or device. `SKILL.md` has the policy and the ELF check.

## Package check

Use `zipalign` from build-tools 35.0.0 or later; older versions do not have `-P 16`.

```bash
ZIPALIGN="$ANDROID_HOME/build-tools/<35.0.0-or-later>/zipalign"
"$ZIPALIGN" -c -P 16 -v 4 app-release.apk            # a final APK

# An AAB: prove the requested alignment, then check every APK in the DEFAULT set.
# Do not pass --mode=universal here.
bundletool dump config --bundle=app-release.aab | grep PAGE_ALIGNMENT_16K
bundletool build-apks --bundle=app-release.aab \
  --output=app-release-default.apks --overwrite
unzip -q app-release-default.apks -d <default-apks-dir>
find <default-apks-dir> -name '*.apk' -print0 \
  | xargs -0 -n1 "$ZIPALIGN" -c -P 16 -v 4
```

A universal APK is a local smoke artifact only. It does not cover the split and standalone APKs
that Play can serve. AGP 8.5.1 and later align uncompressed native libraries at 16 KiB. On an
older AGP, upgrade, or set `packaging.jniLibs.useLegacyPackaging = true` so that the libraries
ship compressed and the device extracts them at install.

## Device check

The static checks do not prove the runtime path. On a 16 KB emulator image or device, run these
commands before you install the app:

```bash
adb shell getconf PAGE_SIZE     # must print 16384
adb shell setprop bionic.linker.16kb.app_compat.enabled false
adb shell setprop pm.16kb.app_compat.disabled true
test "$(adb shell getprop bionic.linker.16kb.app_compat.enabled)" = false
test "$(adb shell getprop pm.16kb.app_compat.disabled)" = true
```

Fail when a read-back differs: `setprop` can fail on a user build without root. On Android 17 or
later, set `fatal` instead of `false` so that a misaligned library aborts at load, and expect
`fatal`. Then run the installed release smoke from `references/release-packaging.md`.
