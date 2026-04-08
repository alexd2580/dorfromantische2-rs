fn main() {
    println!("cargo::rerun-if-changed=src/shader.frag");
    println!("cargo::rerun-if-changed=src/shader.vert");
}
