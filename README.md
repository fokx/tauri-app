## Tauri app

## Notes
### Cannot use dotenvy to hide credentials in `.env` file
```sh
7z x win-exe-installer
7z x tauri-app.exe
# 3d91 is the start of the password string
grep --text 3d91 .rdata
# this is definitely not expected!
# so hardcode it in the code!
```

.rdata
### Android puple notification bar

To remove the notification bar color in a Tauri Android app, need to modify the Android-specific configuration in the AndroidManifest.xml file and the styles in the res/values/styles.xml file.  
Modify `./gen/android/app/src/main/AndroidManifest.xml`: Ensure that the theme is set correctly for activity.  

```
<activity
android:name=".MainActivity"
android:theme="@style/Theme.AppCompat.DayNight.NoActionBar">
<!-- other configurations -->
</activity>
```
alternatives to "Theme.AppCompat.DayNight.NoActionBar":
"Theme.AppCompat.Light.NoActionBar"
"Theme.AppCompat.DayNight"
"Theme.MaterialComponents.DayNight"

see:
https://developer.android.com/develop/ui/views/theming/darktheme


## Troubleshooting
### the proxy client cannot connect to the Internet
The proxy client will prefer IPv6 on the server. Make sure IPv6 works on the server, or disable IPv6.