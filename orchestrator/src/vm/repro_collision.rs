
#[cfg(test)]
mod reproduction_tests {
    use super::*;
    use crate::vm::firewall::FirewallManager;

    #[test]
    fn test_firewall_chain_name_length_overflow() {
        let long_id = "a".repeat(30); // 30 chars
        let manager = FirewallManager::new(long_id.clone());
        let chain_name = manager.chain_name();

        println!("Chain name length: {}", chain_name.len());
        println!("Chain name: {}", chain_name);

        // iptables limit is typically 28 chars.
        // If the code doesn't truncate, this will exceed the limit.
        // IRONCLAW_ (9 chars) + 30 chars = 39 chars.
        assert!(chain_name.len() <= 28, "Chain name exceeds iptables limit of 28 chars");
    }
}
