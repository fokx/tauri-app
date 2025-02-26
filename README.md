## Tauri app

## Notes
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
