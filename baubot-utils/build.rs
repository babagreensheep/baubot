//! Build script to generate env variables

use std::io::Write;

fn main() {
    let manifest_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let secrets_path = manifest_path.join("secrets.env");

    // Initialize env secrets
    match dotenvy::from_path(&secrets_path) {
        Ok(_) => {
            // Create build file
            let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
            let secrets_file_path = out_dir.join("secrets.rs");
            let secrets_file = std::fs::File::create(&secrets_file_path).unwrap();
            let mut file_stream = std::io::BufWriter::new(secrets_file);
            println!(
                "cargo:warning=\"secrets file generated at: {}\"",
                secrets_file_path.to_string_lossy()
            );

            // Generate keys
            for (key, value) in std::env::vars().filter_map(|(key, value)| {
                let key = key.strip_prefix("TELE_")?;
                Some((key.to_string(), value))
            }) {
                match value.parse::<u128>() {
                    Ok(value) => writeln!(file_stream, "pub const {key}: u128 = {value};").unwrap(),
                    Err(_) => {
                        writeln!(file_stream, "pub const {key}: &str = \"{value}\";").unwrap()
                    }
                }
            }

            // Flush stream
            file_stream.flush().unwrap();
        }
        Err(_) => println!(
            r#"cargo:warning="no secrets file found at {}""#,
            secrets_path.to_string_lossy()
        ),
    }

    // Force rerun
    let build_script = manifest_path.join("build.rs");
    println!("cargo:rerun-if-changed={}", build_script.to_string_lossy());
    println!("cargo:rerun-if-changed={}", secrets_path.to_string_lossy());
}
