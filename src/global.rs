use rust_utils::raise::RaiseContext;
use serde::Deserialize;

#[derive(Deserialize, getset::Getters)]
#[serde(deny_unknown_fields)]
#[getset(get = "pub")]
pub struct GlobalGuidance {
    #[serde(default)]
    details: Option<String>,
    #[serde(default)]
    checklist: Vec<String>,
}

impl GlobalGuidance {
    pub fn bundled() -> Self {
        toml::from_str(include_str!("assets/global.toml"))
            .raise_with_context(|| "parsing bundled global guidance".into())
    }
}

#[cfg(test)]
mod tests {
    use super::GlobalGuidance;

    #[test]
    fn bundled_guidance_loads() {
        let guidance = GlobalGuidance::bundled();
        assert!(!guidance.checklist().is_empty());
    }

    #[test]
    fn rejects_summary_and_invalid_checklist() {
        assert!(toml::from_str::<GlobalGuidance>("summary = 'unsupported'").is_err());
        assert!(toml::from_str::<GlobalGuidance>("checklist = [1]").is_err());
        assert!(toml::from_str::<GlobalGuidance>("steps = ['Old guidance']").is_err());
    }
}
