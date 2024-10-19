use rusqlite::{Connection, Result};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;


#[derive(Debug, Serialize, Deserialize)]
pub struct PartitionedData {
    pub blocks: Vec<ShaderBlock>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ShaderBlock {
    pub vertex_data: Vec<f32>,
    pub material_data: Vec<f32>,
}

// Define the structure to hold frame metrics
#[derive(Debug, Serialize, Deserialize)]
pub struct FrameData {
    pub frame_number: u32,
    pub vertex_data: Vec<f32>,  // Vertex data to be passed to shaders
    pub material_data: Vec<f32>, // Material properties (e.g., colors)
}

// Define the structure for video metrics
#[derive(Debug, Serialize, Deserialize)]
pub struct VideoMetrics {
    pub frame_data: Vec<FrameData>,
}

// Define a struct for managing database connections and caching
pub struct DatabaseManager {
    conn: Mutex<Connection>, // Mutex for exclusive access to the connection
    schema_cache: Arc<Mutex<SQLiteAttributeCache>>, // Shared schema cache
}

impl DatabaseManager {
    // Create a new DatabaseManager
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        let schema_cache = Arc::new(Mutex::new(SQLiteAttributeCache::new()));
        
        Ok(Self { conn: Mutex::new(conn), schema_cache })
    }

    // Ingest video metrics in a thread-safe manner
    pub fn ingest_video_metrics(&self) -> Result<VideoMetrics> {
        let conn = self.conn.lock().unwrap(); // Lock the connection for exclusive access
        
        let mut stmt = conn.prepare("SELECT frame_number, vertex_data, material_data FROM video_metrics")?;
        
        let metrics_iter = stmt.query_map([], |row| {
            let vertex_data: String = row.get(1)?; // Assuming vertex_data is stored as a CSV string
            let material_data: String = row.get(2)?; // Assuming material_data is stored as a CSV string

            Ok(FrameData {
                frame_number: row.get(0)?,
                vertex_data: parse_csv(&vertex_data),  // Parse CSV to Vec<f32>
                material_data: parse_csv(&material_data), // Parse CSV to Vec<f32>
            })
        })?;

        let frame_data: Vec<FrameData> = metrics_iter.collect::<Result<Vec<_>, _>>()?;
        Ok(VideoMetrics { frame_data })
    }

    // Additional methods for writing data can be added here, ensuring exclusive access when needed.
}

// Helper function to parse CSV string into Vec<f32>
fn parse_csv(data: &str) -> Vec<f32> {
    data.split(',')
        .filter_map(|s| s.trim().parse::<f32>().ok()) // Filter and parse to f32
        .collect()
}

// Placeholder for SQLiteAttributeCache
#[derive(Debug)]
pub struct SQLiteAttributeCache {
    // Store attribute data
}

impl SQLiteAttributeCache {
    pub fn new() -> Self {
        Self {
            // Initialize cache here
        }
    }

    // Methods for cache management can be added here
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_csv() {
        let csv_data = "1.0,2.0,3.0,4.0";
        let parsed = parse_csv(csv_data);
        assert_eq!(parsed, vec![1.0, 2.0, 3.0, 4.0]);
    }
}




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

// Represents a conic tree
pub struct ConicTree {
    pub root: ConicNode,
}

impl ConicTree {
    pub fn new(root: ConicNode) -> Self {
        Self { root }
    }

    pub fn add_child(&mut self, child: ConicNode) {
        self.root.children.push(child);
    }
}

// Function to build a conic tree from a JSON string
