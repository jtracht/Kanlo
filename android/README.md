# Kanlo TV für Android

1. Installiere die APK auf einem Android-Gerät ab Android 8.
2. Verbinde das Gerät mit dem Netzwerk deiner Kanlo-Instanz.
3. Öffne **Kanlo TV** und gib die Serveradresse ein, etwa `http://kanlo.local` oder die Adresse, die Kanlo am Fernseher anzeigt.
4. Tippe auf **Verbinden**. Die App öffnet die TV-Fernbedienung dieser Instanz. Eine geschützte HTTPS-Instanz verwendet die normale Kanlo-Anmeldung.

Die App benötigt einen Kanlo-Server mit der neuen Fernbedienung. Über **Server wechseln** kannst du eine andere Adresse eintragen. Nutze HTTP im vertrauenswürdigen lokalen Netz und HTTPS für den Zugriff über einen geschützten Proxy.

## APK bauen

Benötigt werden Python 3, ein JDK und die Android SDK Platform 35 mit Build Tools 35.0.1.

```sh
python android/build.py \
  --build-tools "$ANDROID_HOME/build-tools/35.0.1" \
  --platform "$ANDROID_HOME/platforms/android-35/android.jar" \
  --jdk "$JAVA_HOME" \
  --keystore "$ANDROID_HOME/kanlo-preview.p12" \
  --output artifacts/android/kanlo-tv-0.1.0-preview.apk
```

Der Build signiert und verifiziert die Test-APK mit einem lokalen Vorschau-Schlüssel. Für ein Update derselben Installation braucht der nächste Build denselben Schlüssel.
