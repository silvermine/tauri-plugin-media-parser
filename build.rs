const COMMANDS: &[&str] = &["get_metadata"];

fn main() {
   tauri_plugin::Builder::new(COMMANDS).build();
}
