/* -----------------------------------------------------------------------------
 * inject.rs
 * Reads userscripts from disk and injects them into HTML at the configured
 * position.
 * -------------------------------------------------------------------------- */

use tracing::*;

use dope_core::Config;

/* --- Inject Scripts -------------------------------------------------------- */

pub fn inject_scripts(html: &mut String, domain: &str, config: &Config) {
    let scripts = config.get_scripts_for_domain(domain);
    if scripts.is_empty() {
        return;
    }

    let inject_at = config
        .get_response_modifiers(domain)
        .and_then(|m| m.inject_at.as_deref())
        .unwrap_or("body_end");

    for script_name in scripts {
        info!("Injecting script: {} into {}", script_name, domain);

        let script_content = match crate::scripts::read_script(&script_name) {
            Ok(c) => c,
            Err(e) => {
                error!("{}", e);
                continue;
            }
        };

        let tag = if script_content.contains("GM_") {
            let gm_polyfill = include_str!("../../lib/gm_polyfill.js");
            format!(
                "<script>{}</script><script>{}</script>",
                gm_polyfill, script_content
            )
        } else {
            format!("<script>{}</script>", script_content)
        };

        match inject_at {
            "head_end" => {
                if html.contains("</head>") {
                    *html = html.replace("</head>", &format!("{}{}", tag, "</head>"));
                } else if html.contains("<body") {
                    *html = html.replace("<body", &format!("{}{}", tag, "<body"));
                } else {
                    html.push_str(&tag);
                }
            }
            "body_end" => {
                if html.contains("</body>") {
                    *html = html.replace("</body>", &format!("{}{}", tag, "</body>"));
                } else if html.contains("</html>") {
                    *html = html.replace("</html>", &format!("{}{}", tag, "</html>"));
                } else {
                    html.push_str(&tag);
                }
            }
            "html_end" => {
                if html.contains("</html>") {
                    *html = html.replace("</html>", &format!("{}{}", tag, "</html>"));
                } else {
                    html.push_str(&tag);
                }
            }
            "append" => {
                html.push_str(&tag);
            }
            other => {
                warn!("Unknown inject_at position: {}", other);
                html.push_str(&tag);
            }
        }
    }
}
