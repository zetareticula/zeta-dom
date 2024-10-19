#[derive(Debug)]


// Represents a single node in the conic tree
pub struct ConicNode {
    pub name: String,
    pub value: Option<String>, // Holds specific values (if any)
    pub children: Vec<ConicNode>, // Child nodes
}

impl ConicNode {
    pub fn new(name: &str, value: Option<&str>) -> Self {
        Self {
            name: name.to_string(),
            value: value.map(|v| v.to_string()),
            children: Vec::new(),
        }
    }

    pub fn add_child(&mut self, child: ConicNode) {
        self.children.push(child);
    }
}

