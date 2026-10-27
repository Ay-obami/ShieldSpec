pub(crate) fn fingerprint(
    rule_id: &str,
    step_index: usize,
    structural_evidence: &[&str],
) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in rule_id
        .bytes()
        .chain([0xff])
        .chain(step_index.to_string().bytes())
        .chain([0xff])
        .chain(
            structural_evidence
                .iter()
                .flat_map(|part| part.bytes().chain([0xff]).collect::<Vec<_>>().into_iter()),
        )
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("ss-{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::fingerprint;

    #[test]
    fn changes_in_structural_evidence_change_the_fingerprint() {
        assert_ne!(
            fingerprint("SS001", 0, &["output", "0"]),
            fingerprint("SS001", 0, &["output", "1"])
        );
    }
}
