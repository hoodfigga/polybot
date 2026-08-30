use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=schema/telemetry.fbs");

    // First try using system flatc
    let output = Command::new("flatc")
        .args([
            "--rust",
            "-o",
            &std::env::var("OUT_DIR").unwrap(),
            "schema/telemetry.fbs",
        ])
        .status();

    match output {
        Ok(status) if status.success() => {
            println!("cargo:info=Successfully compiled telemetry.fbs using system flatc");
        }
        _ => {
            panic!("flatc compiler execution failed. Please verify that flatc is installed. You can install it using sudo ./setup_system.sh or by installing flatbuffers-compiler.");
        }
    }
}
