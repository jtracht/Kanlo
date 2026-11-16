package de.trachtdigital.kanlo.remote;

import android.app.Activity;
import android.os.Bundle;
import android.text.InputType;
import android.view.ViewGroup;
import android.webkit.CookieManager;
import android.webkit.WebResourceRequest;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.Button;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.TextView;
import java.net.URI;
import java.util.Locale;

public final class MainActivity extends Activity {
    private LinearLayout layout;
    private WebView web;
    private boolean german;

    @Override public void onCreate(Bundle savedState) {
        super.onCreate(savedState);
        german = Locale.getDefault().getLanguage().equals("de");
        String origin = normalize(getPreferences(MODE_PRIVATE).getString("origin", ""));
        if (origin == null) settings(); else open(origin);
    }

    private String label(String de, String en) { return german ? de : en; }

    private void initLayout() {
        if (web != null) {
            layout.removeView(web);
            web.destroy();
            web = null;
        }
        layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        layout.setFitsSystemWindows(true);
        setContentView(layout);
    }

    private void settings() {
        initLayout();
        int padding = (int) (20 * getResources().getDisplayMetrics().density);
        layout.setPadding(padding, padding, padding, padding);
        TextView title = new TextView(this);
        title.setText(label("Kanlo TV verbinden", "Connect Kanlo TV"));
        title.setTextSize(24);
        layout.addView(title);
        EditText address = new EditText(this);
        address.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_URI);
        address.setSingleLine(true);
        address.setHint("http://kanlo.local");
        address.setText(getPreferences(MODE_PRIVATE).getString("origin", ""));
        layout.addView(address);
        Button connect = new Button(this);
        connect.setText(label("Verbinden", "Connect"));
        connect.setOnClickListener(view -> {
            String origin = normalize(address.getText().toString());
            if (origin == null) {
                address.setError(label("HTTP- oder HTTPS-Adresse ohne Pfad eingeben", "Enter an HTTP or HTTPS origin"));
                return;
            }
            getPreferences(MODE_PRIVATE).edit().putString("origin", origin).apply();
            open(origin);
        });
        layout.addView(connect);
    }

    private void open(String origin) {
        initLayout();
        Button edit = new Button(this);
        edit.setText(label("Server wechseln", "Change server"));
        edit.setOnClickListener(view -> settings());
        layout.addView(edit);
        web = new WebView(this);
        web.getSettings().setJavaScriptEnabled(true);
        web.getSettings().setDomStorageEnabled(true);
        web.getSettings().setAllowFileAccess(false);
        web.getSettings().setAllowContentAccess(false);
        web.getSettings().setMixedContentMode(android.webkit.WebSettings.MIXED_CONTENT_NEVER_ALLOW);
        CookieManager.getInstance().setAcceptThirdPartyCookies(web, false);
        web.setWebViewClient(new WebViewClient() {
            @Override public boolean shouldOverrideUrlLoading(WebView view, WebResourceRequest request) {
                return !origin.equals(originOf(request.getUrl().toString()));
            }
        });
        layout.addView(web, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1));
        web.loadUrl(origin + "/?tv-remote");
    }

    private static String normalize(String input) {
        String value = input.trim();
        if (value.isEmpty()) return null;
        if (!value.contains("://")) value = "http://" + value;
        try {
            URI uri = URI.create(value);
            if (uri.getRawQuery() != null || uri.getRawFragment() != null
                || (!uri.getPath().isEmpty() && !uri.getPath().equals("/"))) return null;
            return originOf(value);
        } catch (IllegalArgumentException ex) {
            return null;
        }
    }

    private static String originOf(String value) {
        try {
            URI uri = URI.create(value);
            String scheme = uri.getScheme();
            String host = uri.getHost();
            int port = uri.getPort();
            if (host == null || uri.getUserInfo() != null || port == 0 || port > 65535
                || (!"https".equalsIgnoreCase(scheme) && !"http".equalsIgnoreCase(scheme))) return null;
            scheme = scheme.toLowerCase(Locale.ROOT);
            boolean defaultPort = port == -1 || port == (scheme.equals("https") ? 443 : 80);
            return scheme + "://" + host.toLowerCase(Locale.ROOT) + (defaultPort ? "" : ":" + port);
        } catch (IllegalArgumentException ex) {
            return null;
        }
    }

    @Override protected void onDestroy() {
        if (web != null) {
            layout.removeView(web);
            web.destroy();
        }
        super.onDestroy();
    }
}
