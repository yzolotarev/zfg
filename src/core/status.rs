#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Status {
    pub focus_count: usize,
    pub focus_limit: usize,
    pub focus_first: String,
    pub later_count: usize,
    pub focus_full: bool,
}
