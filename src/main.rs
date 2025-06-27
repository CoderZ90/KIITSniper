use difference::{Changeset, Difference};
use headless_chrome::{Browser, LaunchOptions};
use reqwest::blocking::{multipart, Client};
use std::env;
use std::fs::{read_to_string, File};
use std::io::Read;
use std::io::Write;
use std::time::Duration;

fn main() {
    dotenvy::dotenv().ok(); // load .env

    println!(
        "
                ██╗  ██╗██╗██╗████████╗███████╗███████╗    ███████╗███╗   ██╗██╗██████╗ ███████╗██████╗ 
                ██║ ██╔╝██║██║╚══██╔══╝██╔════╝██╔════╝    ██╔════╝████╗  ██║██║██╔══██╗██╔════╝██╔══██╗
                █████╔╝ ██║██║   ██║   █████╗  █████╗      ███████╗██╔██╗ ██║██║██████╔╝█████╗  ██████╔╝
                ██╔═██╗ ██║██║   ██║   ██╔══╝  ██╔══╝      ╚════██║██║╚██╗██║██║██╔═══╝ ██╔══╝  ██╔══██╗
                ██║  ██╗██║██║   ██║   ███████╗███████╗    ███████║██║ ╚████║██║██║     ███████╗██║  ██║
                ╚═╝  ╚═╝╚═╝╚═╝   ╚═╝   ╚══════╝╚══════╝    ╚══════╝╚═╝  ╚═══╝╚═╝╚═╝     ╚══════╝╚═╝  ╚═╝
                                                                                        
        "
    );
    println!("~ live change detector - Detects the changes made in the website. \n\n");

    let url = "https://kiitee.kiit.ac.in/";
    println!("@ Fetching Content From: {}", url);

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .expect("Failed to build HTTP client");

    let response = client.get(url).send().expect("❌ Failed to send request");
    let html = match response.text() {
        Ok(text) => text,
        Err(e) => {
            eprintln!("❌ Failed to read response text: {}", e);
            return;
        }
    };

    let snapshot_path = "kiit_snapshot.html";
    let old_html = read_to_string(snapshot_path).unwrap_or_else(|_| {
        println!("No Snapshot Found - creating one now (Rerun the program to detect changes)");
        save_snapshot(&html);
        std::process::exit(0);
    });

    let cleaned_current = clean_html(&html);
    let cleaned_old = clean_html(&old_html);

    let diff = Changeset::new(&cleaned_old, &cleaned_current, "\n");

    if diff.diffs.len() == 1 {
        println!("✅ No changes detected");
    } else {
        println!("🔍 Detected Changes: ");

        let mut changes_for_discord = String::new();

        for change in diff.diffs {
            match change {
                Difference::Same(_) => {}
                Difference::Add(add) => {
                    println!("🟢 Added:\n{}\n", add);
                    changes_for_discord.push_str(&format!("🟢 Added:\n{}\n", add));
                }
                Difference::Rem(rem) => {
                    println!("🔴 Removed:\n{}\n", rem);
                    changes_for_discord.push_str(&format!("🔴 Removed:\n{}\n", rem));
                }
            }
        }

        save_snapshot(&html);
        println!("💾 Snapshot Updated");

        // 🔔 Notify Discord
        let screenshot_path = "kiit_screenshot.png";
        if let Err(e) = take_screenshot(url, screenshot_path) {
            println!("⚠️ Screenshot failed: {}", e);
            send_discord_notification(&changes_for_discord); // fallback
        } else {
            send_discord_notification_with_screenshot(&changes_for_discord, screenshot_path);
        }
    }

    #[cfg(not(debug_assertions))]
    println!("\n🕒 Closing in 30 seconds...");
    #[cfg(not(debug_assertions))]
    std::thread::sleep(Duration::from_secs(30));
}

fn take_screenshot(url: &str, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let browser = Browser::new(LaunchOptions::default_builder().headless(true).build()?)?;
    let tab = browser.new_tab()?;
    tab.navigate_to(url)?;
    tab.wait_for_element("body")?;
    std::thread::sleep(Duration::from_secs(3)); // give time for full load

    let png_data = tab.capture_screenshot(
        headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption::Png,
        None,
        None,
        true,
    )?;

    std::fs::write(path, &png_data)?;
    Ok(())
}

fn send_discord_notification_with_screenshot(message: &str, image_path: &str) {
    let webhook_url = match env::var("DISCORD_WEBHOOK") {
        Ok(url) => url,
        Err(_) => {
            println!("⚠️ DISCORD_WEBHOOK not set.");
            return;
        }
    };

    // Load image
    let mut file = File::open(image_path).expect("Could not open screenshot.png");
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).unwrap();

    let form = multipart::Form::new()
        .text(
            "content",
            format!("🔔 **KIIT Website Changed!**\n{}", message),
        )
        .part(
            "file",
            multipart::Part::bytes(buf)
                .file_name("screenshot.png")
                .mime_str("image/png")
                .unwrap(),
        );

    let client = Client::new();
    let res = client.post(webhook_url).multipart(form).send();

    match res {
        Ok(_) => println!("📸 Screenshot + message sent to Discord"),
        Err(e) => println!("❌ Discord error: {}", e),
    }
}

/// Sends a message to Discord via Webhook
fn send_discord_notification(message: &str) {
    let webhook_url = env::var("DISCORD_WEBHOOK").unwrap_or_else(|_| {
        println!("⚠️ DISCORD_WEBHOOK not set. Skipping notification.");
        return String::new();
    });

    if webhook_url.is_empty() {
        return;
    }

    let payload = serde_json::json!({
        "content": format!("🔔 **KIIT Website Changed!**\n{}", message)
    });

    let client = Client::new();
    let response = client.post(&webhook_url).json(&payload).send();

    match response {
        Ok(_) => println!("📣 Discord notification sent!"),
        Err(e) => println!("❌ Failed to send Discord notification: {}", e),
    }
}

fn save_snapshot(html: &str) {
    let mut file = File::create("kiit_snapshot.html").expect("Failed to save snapshot.html");
    file.write_all(html.as_bytes())
        .expect("Failed to write file");

    println!("📁 Snapshot saved successfully");
}

fn clean_html(input: &str) -> String {
    let mut cleaned = input.to_string();
    cleaned = cleaned.replace("__cf_email__", "");

    let script_tag_start = "window.__CF$cv$params";
    if let Some(pos) = cleaned.find(script_tag_start) {
        if let Some(end) = cleaned[pos..].find("</script>") {
            cleaned.replace_range(pos..pos + end + 9, "");
        }
    }

    cleaned
}
