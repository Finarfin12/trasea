//! Internal owner utility (NOT bundled into installer) for Ed25519 offline license keys.
//!
//! Usage:
//!   trasea-keygen init
//!   trasea-keygen issue --secret <HEX_OR_ENV> --name "Nama Pembeli" --id "TR-2026-0001" [--tier pro] [--issued YYYY-MM-DD]
//!   trasea-keygen verify <TRSA1-...> [--pubkey <HEX>]

use trasea_core::license::{from_hex_32, generate_keypair, sign_license, to_hex, verify_embedded, verify_license, LicenseInfo};

fn usage() {
    eprintln!(
        "usage:
  trasea-keygen init
  trasea-keygen issue --secret <HEX> --name <NAME> --id <ID> [--tier pro] [--issued YYYY-MM-DD]
  trasea-keygen verify <KEY> [--pubkey <HEX>]"
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first().map(String::as_str) else {
        usage();
        std::process::exit(2);
    };
    match cmd {
        "init" => {
            let (secret, pubkey) = generate_keypair().unwrap_or_else(|e| {
                eprintln!("error: {e}");
                std::process::exit(1);
            });
            let rust_bytes: Vec<String> = pubkey.iter().map(|b| format!("0x{b:02x}")).collect();
            println!("PUBLIC_KEY_HEX={}", to_hex(&pubkey));
            println!("PRIVATE_KEY_HEX={}", to_hex(&secret));
            println!("EMBEDDED_PUBLIC_KEY=[{}]", rust_bytes.join(", "));
        }
        "issue" => {
            let mut secret_hex = std::env::var("TRASEA_LICENSE_SECRET").ok();
            let mut name: Option<String> = None;
            let mut id: Option<String> = None;
            let mut tier = "pro".to_string();
            let mut issued = "2026-11-23".to_string();
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--secret" => { i += 1; secret_hex = args.get(i).cloned(); }
                    "--name" => { i += 1; name = args.get(i).cloned(); }
                    "--id" => { i += 1; id = args.get(i).cloned(); }
                    "--tier" => { i += 1; if let Some(v) = args.get(i) { tier = v.clone(); } }
                    "--issued" => { i += 1; if let Some(v) = args.get(i) { issued = v.clone(); } }
                    other => {
                        eprintln!("unknown option {other}");
                        std::process::exit(2);
                    }
                }
                i += 1;
            }
            let secret = secret_hex
                .ok_or_else(|| "berikan --secret <HEX> atau env TRASEA_LICENSE_SECRET".to_string())
                .and_then(|h| from_hex_32(&h))
                .unwrap_or_else(|e| { eprintln!("error: {e}"); std::process::exit(2); });
            let name = name.unwrap_or_else(|| { eprintln!("--name wajib diisi"); std::process::exit(2); });
            let id = id.unwrap_or_else(|| { eprintln!("--id wajib diisi"); std::process::exit(2); });
            let info = LicenseInfo { v: 1, id, name, tier, issued };
            let key = sign_license(&info, &secret).unwrap_or_else(|e| { eprintln!("error: {e}"); std::process::exit(1); });
            println!("{key}");
        }
        "verify" => {
            let Some(key) = args.get(1) else {
                usage();
                std::process::exit(2);
            };
            let mut pub_override: Option<String> = None;
            let mut i = 2;
            while i < args.len() {
                if args[i] == "--pubkey" {
                    i += 1;
                    pub_override = args.get(i).cloned();
                } else {
                    eprintln!("unknown option {}", args[i]);
                    std::process::exit(2);
                }
                i += 1;
            }
            let res = match pub_override {
                Some(hex) => {
                    let pk = from_hex_32(&hex).unwrap_or_else(|e| { eprintln!("error: {e}"); std::process::exit(2); });
                    verify_license(key, &pk)
                }
                None => verify_embedded(key),
            };
            match res {
                Ok(info) => println!("{}", serde_json::to_string_pretty(&info).unwrap()),
                Err(e) => {
                    eprintln!("INVALID: {e}");
                    std::process::exit(1);
                }
            }
        }
        _ => {
            usage();
            std::process::exit(2);
        }
    }
}
