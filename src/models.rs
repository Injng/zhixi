use rocket::serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Deserialize, Serialize, FromRow)]
#[serde(crate = "rocket::serde")]
pub struct Semester {
    pub id: i64,
    pub name: String,
    pub created_at: String, // Simplified for now, can use chrono if needed
}

#[derive(Debug, Clone, Deserialize, Serialize, FromRow)]
#[serde(crate = "rocket::serde")]
pub struct Course {
    pub id: i64,
    pub semester_id: i64,
    pub code: String,
    pub title: String,
    pub is_published: bool,
    pub public_slug: Option<String>,
    pub show_lecture_links: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, FromRow)]
#[serde(crate = "rocket::serde")]
pub struct LogItem {
    pub id: i64,
    pub course_id: i64,
    pub kind: String,
    pub title: String,
    pub description: Option<String>,
    pub link: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, FromRow)]
#[serde(crate = "rocket::serde")]
pub struct Category {
    pub id: i64,
    pub course_id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, FromRow)]
#[serde(crate = "rocket::serde")]
pub struct Exam {
    pub id: i64,
    pub course_id: i64,
    pub title: String,
    pub semester: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, FromRow)]
#[serde(crate = "rocket::serde")]
pub struct Problem {
    pub id: i64,
    pub log_item_id: Option<i64>,
    pub exam_id: Option<i64>,
    pub description: String,
    pub notes: Option<String>,
    pub image_url: Option<String>,
    pub solution_link: Option<String>,
    pub is_incorrect: bool,
}

// Helper struct for joining problems with their categories
#[derive(Debug, Clone, Deserialize, Serialize, FromRow)]
#[serde(crate = "rocket::serde")]
pub struct ProblemWithCategories {
    pub id: i64,
    pub log_item_id: Option<i64>,
    pub exam_id: Option<i64>,
    pub description: String,
    pub notes: Option<String>,
    pub image_url: Option<String>,
    pub solution_link: Option<String>,
    pub is_incorrect: bool,
    pub category_names: Option<String>, // Comma separated list from group_concat
    pub source_kind: String, // From joined log_item
    pub source_title: String, // From joined log_item
}

#[derive(Debug, Clone, Deserialize, Serialize, FromRow)]
#[serde(crate = "rocket::serde")]
pub struct User {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
}

// Public-facing structs (not FromRow — constructed in Rust logic)

#[derive(Debug, Clone, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct PublicLogItem {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub description: Option<String>,
    pub date: Option<String>,
    pub link: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct PublicProblem {
    pub id: i64,
    pub image_url: Option<String>,
    pub notes: Option<String>,
    pub category_names: Option<String>,
    pub source_kind: String,
    pub source_title: String,
    pub solution_link: Option<String>,
}

pub struct CalendarWeek {
    pub week_number: u32,
    pub start_date: String,
    pub end_date: String,
    pub items_by_kind: Vec<(String, Vec<PublicLogItem>)>,
}
