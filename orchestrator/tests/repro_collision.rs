use ironclaw_orchestrator::vm::firewall::FirewallManager;

#[test]
#[ignore = "Demonstrates collision vulnerability - fails until fixed"]
fn test_firewall_collision() {
    let vm1 = "test-vm".to_string();
    let vm2 = "test_vm".to_string();

    let mgr1 = FirewallManager::new(vm1.clone());
    let mgr2 = FirewallManager::new(vm2.clone());

    let chain1 = mgr1.chain_name();
    let chain2 = mgr2.chain_name();

    println!("VM1: {} -> Chain: {}", vm1, chain1);
    println!("VM2: {} -> Chain: {}", vm2, chain2);

    // This assertion SHOULD FAIL if there is a collision
    assert_ne!(
        chain1, chain2,
        "Collision detected! Different VM IDs mapped to same chain name."
    );
}

#[test]
#[ignore = "Demonstrates length vulnerability - fails until fixed"]
fn test_firewall_chain_length() {
    let long_id = "this-is-a-very-long-vm-id-that-exceeds-the-limit-of-iptables".to_string();
    let mgr = FirewallManager::new(long_id.clone());
    let chain = mgr.chain_name();

    println!(
        "Long ID: {} -> Chain: {} (len: {})",
        long_id,
        chain,
        chain.len()
    );

    // iptables chain name limit is usually 28 characters
    assert!(
        chain.len() <= 28,
        "Chain name too long! {} chars (max 28)",
        chain.len()
    );
}
