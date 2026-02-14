fn main() {
    let id1 = "long-project-task-name-1";
    let sanitized: String = id1
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .take(19)
        .collect();
    
    let chain_name = format!("LUMINAGUARD_{}", sanitized);
    println!("ID: {}", id1);
    println!("Sanitized: {} (len={})", sanitized, sanitized.len());
    println!("Chain name: {} (len={})", chain_name, chain_name.len());
    
    println!("\nTest with id3:");
    let id3 = "different-long-id-xyz-1";
    let sanitized3: String = id3
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .take(19)
        .collect();
    let chain3 = format!("LUMINAGUARD_{}", sanitized3);
    println!("ID: {}", id3);
    println!("Sanitized: {} (len={})", sanitized3, sanitized3.len());
    println!("Chain name: {} (len={})", chain3, chain3.len());
}
