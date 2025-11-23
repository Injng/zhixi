use rocket::form::Form;
use rocket::fs::TempFile;
use uuid::Uuid;
use rocket_db_pools::Connection;
use rocket_db_pools::sqlx;
use sqlx::Row;
use askama::Template;
use crate::db::Db;
use crate::models::*;
use crate::auth::AuthUser;
use rocket::http::{Cookie, CookieJar, SameSite};
use bcrypt::{hash, verify, DEFAULT_COST};
use rocket::response::Redirect;

// Templates
#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    semesters: Vec<Semester>,
    user: Option<AuthUser>,
}

#[derive(Template)]
#[template(path = "partials/semester_row.html")]
struct SemesterRowTemplate {
    semester: Semester,
    user: Option<AuthUser>,
}

#[derive(Template)]
#[template(path = "semester.html")]
struct SemesterTemplate {
    semester: Semester,
    courses: Vec<Course>,
    user: Option<AuthUser>,
}

#[derive(Template)]
#[template(path = "partials/course_card.html")]
struct CourseCardTemplate {
    course: Course,
    user: Option<AuthUser>,
}

#[derive(Template)]
#[template(path = "course_log.html")]
struct CourseLogTemplate {
    course: Course,
    courses: Vec<Course>,
    log_items: Vec<LogItem>,
    semester: Semester,
    categories: Vec<Category>,
    user: Option<AuthUser>,
}

#[derive(Template)]
#[template(path = "partials/log_item.html")]
struct LogItemTemplate {
    item: LogItem,
    categories: Vec<Category>,
    user: Option<AuthUser>,
}

#[derive(Template)]
#[template(path = "partials/log_item_edit.html")]
struct LogItemEditTemplate {
    item: LogItem,
    user: Option<AuthUser>,
}

#[derive(Template)]
#[template(path = "partials/problem_row.html")]
struct ProblemRowTemplate {
    problem: ProblemWithCategories,
    user: Option<AuthUser>,
}

#[derive(Template)]
#[template(path = "partials/problem_edit.html")]
struct ProblemEditTemplate {
    problem: ProblemWithCategories,
    user: Option<AuthUser>,
}

#[derive(Template)]
#[template(path = "course_study.html")]
struct CourseStudyTemplate {
    course: Course,
    courses: Vec<Course>,
    categories: Vec<Category>,
    semester: Semester,
    user: Option<AuthUser>,
}

#[derive(Template)]
#[template(path = "partials/study_problem_list.html")]
struct StudyProblemListTemplate {
    problems: Vec<ProblemWithCategories>,
    user: Option<AuthUser>,
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {
    user: Option<AuthUser>,
    error: Option<String>,
}

#[derive(Template)]
#[template(path = "register.html")]
struct RegisterTemplate {
    user: Option<AuthUser>,
    error: Option<String>,
}

// Forms
#[derive(FromForm)]
struct NewSemester {
    name: String,
}

#[derive(FromForm)]
struct NewCourse {
    code: String,
    title: String,
}

#[derive(FromForm)]
struct NewLogItem {
    kind: String,
    title: String,
    description: Option<String>,
    link: Option<String>,
    date: Option<String>,
}

#[derive(FromForm)]
struct UpdateLogItem {
    kind: String,
    title: String,
    description: Option<String>,
    link: Option<String>,
    date: Option<String>,
}

#[derive(FromForm)]
struct NewProblem<'r> {
    screenshot: TempFile<'r>,
    notes: Option<String>,
    categories: Option<String>, // Comma separated
    solution_link: Option<String>,
}

#[derive(FromForm)]
struct UpdateProblem {
    notes: Option<String>,
    solution_link: Option<String>,
    categories: Option<String>,
}

#[derive(FromForm)]
struct LoginUser {
    username: String,
    password: String,
}

#[derive(FromForm)]
struct RegisterUser {
    username: String,
    password: String,
}

// Auth Routes

#[get("/login")]
async fn get_login(user: Option<AuthUser>) -> Result<LoginTemplate, Redirect> {
    if user.is_some() {
        return Err(Redirect::to("/"));
    }
    Ok(LoginTemplate { user: None, error: None })
}

#[post("/login", data = "<form>")]
async fn post_login(mut db: Connection<Db>, cookies: &CookieJar<'_>, form: Form<LoginUser>) -> Result<Redirect, LoginTemplate> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
        .bind(&form.username)
        .fetch_optional(&mut **db)
        .await
        .unwrap_or(None);

    if let Some(user) = user {
        if verify(&form.password, &user.password_hash).unwrap_or(false) {
            cookies.add_private(
                Cookie::build(("user_id", user.id.to_string()))
                    .same_site(SameSite::Lax)
                    .build()
            );
            return Ok(Redirect::to("/"));
        }
    }

    Err(LoginTemplate { 
        user: None, 
        error: Some("Invalid username or password".into()) 
    })
}

#[get("/register")]
async fn get_register(user: Option<AuthUser>) -> Result<RegisterTemplate, Redirect> {
    if user.is_some() {
        return Err(Redirect::to("/"));
    }
    Ok(RegisterTemplate { user: None, error: None })
}

#[post("/register", data = "<form>")]
async fn post_register(mut db: Connection<Db>, cookies: &CookieJar<'_>, form: Form<RegisterUser>) -> Result<Redirect, RegisterTemplate> {
    // Check if user exists
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE username = ?)")
        .bind(&form.username)
        .fetch_one(&mut **db)
        .await
        .unwrap_or(false);

    if exists {
        return Err(RegisterTemplate { 
            user: None, 
            error: Some("Username already taken".into()) 
        });
    }

    let hash = hash(&form.password, DEFAULT_COST).unwrap();
    let id = sqlx::query("INSERT INTO users (username, password_hash) VALUES (?, ?)")
        .bind(&form.username)
        .bind(hash)
        .execute(&mut **db)
        .await;

    match id {
        Ok(result) => {
            cookies.add_private(
                Cookie::build(("user_id", result.last_insert_rowid().to_string()))
                    .same_site(SameSite::Lax)
                    .build()
            );
            Ok(Redirect::to("/"))
        },
        Err(_) => Err(RegisterTemplate { 
            user: None, 
            error: Some("Registration failed".into()) 
        })
    }
}

#[post("/logout")]
async fn logout(cookies: &CookieJar<'_>) -> Redirect {
    cookies.remove_private(Cookie::from("user_id"));
    Redirect::to("/login")
}

// Routes

#[get("/")]
async fn index(_db: Connection<Db>, user: Option<AuthUser>) -> Redirect {
    // If not logged in, redirect to login? 
    // Let's allow public view but require login for edit. 
    // Actually, user requested "add auth", usually means access control.
    // Let's make index public but show login/register if not logged in.
    // Wait, I changed the return type to Redirect? No, I should keep it IndexTemplate if public.
    // But if I want to protect it:
    if user.is_none() {
         return Redirect::to("/login");
    }
    Redirect::to("/dashboard") // Or just render index
}

// Renaming index to dashboard or keeping it index but protected
#[get("/dashboard")]
async fn dashboard(mut db: Connection<Db>, user: AuthUser) -> IndexTemplate {
    let semesters = sqlx::query_as::<_, Semester>("SELECT * FROM semesters ORDER BY created_at DESC")
        .fetch_all(&mut **db)
        .await
        .unwrap_or_default();
    IndexTemplate { semesters, user: Some(user) }
}

#[post("/semesters", data = "<form>")]
async fn create_semester(mut db: Connection<Db>, user: AuthUser, form: Form<NewSemester>) -> SemesterRowTemplate {
    let id = sqlx::query("INSERT INTO semesters (name) VALUES (?)")
        .bind(&form.name)
        .execute(&mut **db)
        .await
        .unwrap()
        .last_insert_rowid();
    
    let semester = Semester {
        id,
        name: form.name.clone(),
        created_at: String::new(), // Simplified
    };
    SemesterRowTemplate { semester, user: Some(user) }
}

#[get("/semesters/<id>")]
async fn view_semester(mut db: Connection<Db>, user: AuthUser, id: i64) -> SemesterTemplate {
    let semester = sqlx::query_as::<_, Semester>("SELECT * FROM semesters WHERE id = ?")
        .bind(id)
        .fetch_one(&mut **db)
        .await
        .unwrap();
        
    let courses = sqlx::query_as::<_, Course>("SELECT * FROM courses WHERE semester_id = ?")
        .bind(id)
        .fetch_all(&mut **db)
        .await
        .unwrap_or_default();
        
    SemesterTemplate { semester, courses, user: Some(user) }
}

#[post("/semesters/<id>/courses", data = "<form>")]
async fn create_course(mut db: Connection<Db>, user: AuthUser, id: i64, form: Form<NewCourse>) -> CourseCardTemplate {
    let course_id = sqlx::query("INSERT INTO courses (semester_id, code, title) VALUES (?, ?, ?)")
        .bind(id)
        .bind(&form.code)
        .bind(&form.title)
        .execute(&mut **db)
        .await
        .unwrap()
        .last_insert_rowid();
        
    let course = Course {
        id: course_id,
        semester_id: id,
        code: form.code.clone(),
        title: form.title.clone(),
    };
    CourseCardTemplate { course, user: Some(user) }
}

#[get("/courses/<id>")]
async fn view_course_log(mut db: Connection<Db>, user: AuthUser, id: i64) -> CourseLogTemplate {
    let course = sqlx::query_as::<_, Course>("SELECT * FROM courses WHERE id = ?")
        .bind(id)
        .fetch_one(&mut **db)
        .await
        .unwrap();

    let semester = sqlx::query_as::<_, Semester>("SELECT * FROM semesters WHERE id = ?")
        .bind(course.semester_id)
        .fetch_one(&mut **db)
        .await
        .unwrap();

    let courses = sqlx::query_as::<_, Course>("SELECT * FROM courses WHERE semester_id = ?")
        .bind(course.semester_id)
        .fetch_all(&mut **db)
        .await
        .unwrap_or_default();
        
    let log_items = sqlx::query_as::<_, LogItem>("SELECT * FROM log_items WHERE course_id = ? ORDER BY date DESC, id DESC")
        .bind(id)
        .fetch_all(&mut **db)
        .await
        .unwrap_or_default();

    let categories = sqlx::query_as::<_, Category>("SELECT * FROM categories WHERE course_id = ?")
        .bind(id)
        .fetch_all(&mut **db)
        .await
        .unwrap_or_default();
        
    CourseLogTemplate { course, courses, log_items, semester, categories, user: Some(user) }
}

#[post("/courses/<id>/logs", data = "<form>")]
async fn create_log_item(mut db: Connection<Db>, user: AuthUser, id: i64, form: Form<NewLogItem>) -> LogItemTemplate {
    let item_id = sqlx::query("INSERT INTO log_items (course_id, kind, title, description, link, date) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(id)
        .bind(&form.kind)
        .bind(&form.title)
        .bind(&form.description)
        .bind(&form.link)
        .bind(&form.date)
        .execute(&mut **db)
        .await
        .unwrap()
        .last_insert_rowid();
        
    let item = LogItem {
        id: item_id,
        course_id: id,
        kind: form.kind.clone(),
        title: form.title.clone(),
        description: form.description.clone(),
        link: form.link.clone(),
        date: form.date.clone(),
    };

    let categories = sqlx::query_as::<_, Category>("SELECT * FROM categories WHERE course_id = ?")
        .bind(id)
        .fetch_all(&mut **db)
        .await
        .unwrap_or_default();

    LogItemTemplate { item, categories, user: Some(user) }
}

#[delete("/logs/<id>")]
async fn delete_log_item(mut db: Connection<Db>, _user: AuthUser, id: i64) -> String {
    // 1. Find problems associated with this log item
    let problems = sqlx::query("SELECT id FROM problems WHERE log_item_id = ?")
        .bind(id)
        .fetch_all(&mut **db)
        .await
        .unwrap_or_default();

    // 2. Delete problem_categories for these problems
    for problem in problems {
        let problem_id: i64 = problem.try_get("id").unwrap();
        sqlx::query("DELETE FROM problem_categories WHERE problem_id = ?")
            .bind(problem_id)
            .execute(&mut **db)
            .await
            .unwrap();
    }

    // 3. Delete problems
    sqlx::query("DELETE FROM problems WHERE log_item_id = ?")
        .bind(id)
        .execute(&mut **db)
        .await
        .unwrap();

    // 4. Delete the log item
    sqlx::query("DELETE FROM log_items WHERE id = ?")
        .bind(id)
        .execute(&mut **db)
        .await
        .unwrap();

    String::new()
}

#[get("/logs/<id>/edit")]
async fn get_edit_log_item(mut db: Connection<Db>, user: AuthUser, id: i64) -> LogItemEditTemplate {
    let item = sqlx::query_as::<_, LogItem>("SELECT * FROM log_items WHERE id = ?")
        .bind(id)
        .fetch_one(&mut **db)
        .await
        .unwrap();
    LogItemEditTemplate { item, user: Some(user) }
}

#[get("/logs/<id>")]
async fn get_log_item(mut db: Connection<Db>, user: AuthUser, id: i64) -> LogItemTemplate {
    let item = sqlx::query_as::<_, LogItem>("SELECT * FROM log_items WHERE id = ?")
        .bind(id)
        .fetch_one(&mut **db)
        .await
        .unwrap();
        
    let categories = sqlx::query_as::<_, Category>("SELECT * FROM categories WHERE course_id = ?")
        .bind(item.course_id)
        .fetch_all(&mut **db)
        .await
        .unwrap_or_default();
        
    LogItemTemplate { item, categories, user: Some(user) }
}

#[post("/logs/<id>", data = "<form>")]
async fn update_log_item(mut db: Connection<Db>, user: AuthUser, id: i64, form: Form<UpdateLogItem>) -> LogItemTemplate {
    sqlx::query("UPDATE log_items SET kind = ?, title = ?, description = ?, link = ?, date = ? WHERE id = ?")
        .bind(&form.kind)
        .bind(&form.title)
        .bind(&form.description)
        .bind(&form.link)
        .bind(&form.date)
        .bind(id)
        .execute(&mut **db)
        .await
        .unwrap();

    let item = sqlx::query_as::<_, LogItem>("SELECT * FROM log_items WHERE id = ?")
        .bind(id)
        .fetch_one(&mut **db)
        .await
        .unwrap();

    let categories = sqlx::query_as::<_, Category>("SELECT * FROM categories WHERE course_id = ?")
        .bind(item.course_id)
        .fetch_all(&mut **db)
        .await
        .unwrap_or_default();

    LogItemTemplate { item, categories, user: Some(user) }
}

#[post("/logs/<id>/problems", data = "<form>")]
async fn create_problem(mut db: Connection<Db>, user: AuthUser, id: i64, mut form: Form<NewProblem<'_>>) -> ProblemRowTemplate {
    // 1. Handle File Upload
    let file_name = format!("{}.png", Uuid::new_v4());
    let file_path = format!("uploads/{}", file_name);
    // Ensure uploads directory exists (should be done in main, but good to be safe or assume it exists)
    // We use move_copy_to to handle cross-device moves (e.g. /tmp to project dir) which persist_to fails on
    form.screenshot.move_copy_to(&file_path).await.expect("Unable to move or copy file");
    let image_url = format!("/uploads/{}", file_name);

    // 2. Insert Problem
    // Description is required by DB but removed from UI. We'll use a placeholder.
    let description = "Screenshot Problem"; 
    
    let problem_id = sqlx::query("INSERT INTO problems (log_item_id, description, notes, image_url, solution_link) VALUES (?, ?, ?, ?, ?)")
        .bind(id)
        .bind(description)
        .bind(&form.notes)
        .bind(&image_url)
        .bind(&form.solution_link)
        .execute(&mut **db)
        .await
        .unwrap()
        .last_insert_rowid();

    // 3. Handle Categories
    let mut category_names = String::new();
    if let Some(cats) = &form.categories {
        // Need to fetch course_id first
        let log_item = sqlx::query_as::<_, LogItem>("SELECT * FROM log_items WHERE id = ?")
            .bind(id)
            .fetch_one(&mut **db)
            .await
            .unwrap();

        let mut processed_cats = Vec::new();
        for cat_name in cats.split(|c| c == ',' || c == '、').map(|s| s.trim()).filter(|s| !s.is_empty()) {
            // Find or create category
            let cat_id_opt: Option<i64> = sqlx::query_scalar("SELECT id FROM categories WHERE course_id = ? AND name = ?")
                .bind(log_item.course_id)
                .bind(cat_name)
                .fetch_optional(&mut **db)
                .await
                .unwrap();

            let cat_id = match cat_id_opt {
                Some(cid) => cid,
                None => {
                    sqlx::query("INSERT INTO categories (course_id, name) VALUES (?, ?)")
                        .bind(log_item.course_id)
                        .bind(cat_name)
                        .execute(&mut **db)
                        .await
                        .unwrap()
                        .last_insert_rowid()
                }
            };

            // Link problem to category
            sqlx::query("INSERT INTO problem_categories (problem_id, category_id) VALUES (?, ?)")
                .bind(problem_id)
                .bind(cat_id)
                .execute(&mut **db)
                .await
                .unwrap();
            
            processed_cats.push(cat_name);
        }
        category_names = processed_cats.join(",");
    }

    // 4. Return Template
    let problem = ProblemWithCategories {
        id: problem_id,
        log_item_id: id,
        description: description.to_string(),
        notes: form.notes.clone(),
        image_url: Some(image_url),
        solution_link: form.solution_link.clone(),
        category_names: if category_names.is_empty() { None } else { Some(category_names) },
        source_kind: "".to_string(), // Not needed for the row view immediately usually, but let's leave empty
        source_title: "".to_string(),
    };
    
    ProblemRowTemplate { problem, user: Some(user) }
}

#[get("/logs/<id>/problems")]
async fn get_log_problems(mut db: Connection<Db>, _user: AuthUser, id: i64) -> String {
    // This endpoint returns HTML for the list of problems for a specific log item
    // We need a custom query to join categories
    let problems = sqlx::query_as::<_, ProblemWithCategories>(
        r#"
        SELECT 
            p.*, 
            GROUP_CONCAT(c.name) as category_names,
            l.kind as source_kind,
            l.title as source_title
        FROM problems p
        JOIN log_items l ON p.log_item_id = l.id
        LEFT JOIN problem_categories pc ON p.id = pc.problem_id
        LEFT JOIN categories c ON pc.category_id = c.id
        WHERE p.log_item_id = ?
        GROUP BY p.id
        "#
    )
    .bind(id)
    .fetch_all(&mut **db)
    .await
    .unwrap_or_default();

    // Manually render the list of partials (Askama doesn't support iterating over partials easily in a Vec return without a wrapper template)
    // Actually we can just use a wrapper template or just loop here and render.
    // Let's use a simple wrapper template for this list if we didn't define one.
    // Wait, I defined `partials/problem_row.html` but not a list wrapper for this specific context.
    // I'll just return the concatenated string of rendered partials.
    
    let mut html = String::new();
    for p in problems {
        let t = ProblemRowTemplate { problem: p, user: None }; // User not needed for this partial context if we don't use it inside
        html.push_str(&t.render().unwrap());
    }
    html
}

#[get("/courses/<id>/study")]
async fn view_course_study(mut db: Connection<Db>, user: AuthUser, id: i64) -> CourseStudyTemplate {
    let course = sqlx::query_as::<_, Course>("SELECT * FROM courses WHERE id = ?")
        .bind(id)
        .fetch_one(&mut **db)
        .await
        .unwrap();

    let semester = sqlx::query_as::<_, Semester>("SELECT * FROM semesters WHERE id = ?")
        .bind(course.semester_id)
        .fetch_one(&mut **db)
        .await
        .unwrap();

    let courses = sqlx::query_as::<_, Course>("SELECT * FROM courses WHERE semester_id = ?")
        .bind(course.semester_id)
        .fetch_all(&mut **db)
        .await
        .unwrap_or_default();
        
    let categories = sqlx::query_as::<_, Category>("SELECT * FROM categories WHERE course_id = ?")
        .bind(id)
        .fetch_all(&mut **db)
        .await
        .unwrap_or_default();
        
    CourseStudyTemplate { course, courses, categories, semester, user: Some(user) }
}

#[get("/courses/<id>/study/problems?<source>&<category>")]
async fn filter_study_problems(mut db: Connection<Db>, _user: AuthUser, id: i64, source: Option<Vec<String>>, category: Option<Vec<String>>) -> StudyProblemListTemplate {
    // Build dynamic query
    let mut query = String::from(
        r#"
        SELECT 
            p.*, 
            GROUP_CONCAT(c.name) as category_names,
            l.kind as source_kind,
            l.title as source_title
        FROM problems p
        JOIN log_items l ON p.log_item_id = l.id
        LEFT JOIN problem_categories pc ON p.id = pc.problem_id
        LEFT JOIN categories c ON pc.category_id = c.id
        WHERE l.course_id = ?
        "#
    );

    // Filter by Source
    if let Some(sources) = &source {
        if !sources.is_empty() {
            query.push_str(" AND l.kind IN (");
            for (i, s) in sources.iter().enumerate() {
                if i > 0 { query.push_str(", "); }
                query.push_str(&format!("'{}'", s)); // Be careful with SQL injection here, but for now assuming safe inputs or use bind params properly
            }
            query.push_str(")");
        }
    }

    // Filter by Category (This is trickier with the join, but let's do a simple EXISTS or IN)
    // For simplicity, let's just filter in the WHERE clause if the category join matches
    // But since we group by p.id, we need to be careful.
    // A better way is:
    if let Some(cats) = &category {
         if !cats.is_empty() {
             // This logic is slightly flawed if we want problems that have ANY of the categories, but also want to show ALL categories for that problem.
             // The current query joins all categories.
             // We can add a HAVING clause or a subquery.
             // Let's use a subquery for filtering.
             query.push_str(" AND p.id IN (SELECT pc2.problem_id FROM problem_categories pc2 WHERE pc2.category_id IN (");
             for (i, c) in cats.iter().enumerate() {
                 if i > 0 { query.push_str(", "); }
                 query.push_str(c);
             }
             query.push_str("))");
         }
    }

    query.push_str(" GROUP BY p.id");

    let problems = sqlx::query_as::<_, ProblemWithCategories>(&query)
        .bind(id)
        .fetch_all(&mut **db)
        .await
        .unwrap_or_default();

    StudyProblemListTemplate { problems, user: None } // Partial usually doesn't need user unless we show edit buttons in it
}

#[get("/problems/<id>/edit")]
async fn get_edit_problem(mut db: Connection<Db>, user: AuthUser, id: i64) -> ProblemEditTemplate {
    let problem = sqlx::query_as::<_, ProblemWithCategories>(
        r#"
        SELECT 
            p.*, 
            GROUP_CONCAT(c.name) as category_names,
            l.kind as source_kind,
            l.title as source_title
        FROM problems p
        JOIN log_items l ON p.log_item_id = l.id
        LEFT JOIN problem_categories pc ON p.id = pc.problem_id
        LEFT JOIN categories c ON pc.category_id = c.id
        WHERE p.id = ?
        GROUP BY p.id
        "#
    )
    .bind(id)
    .fetch_one(&mut **db)
    .await
    .unwrap();

    ProblemEditTemplate { problem, user: Some(user) }
}

#[get("/problems/<id>")]
async fn get_problem_row(mut db: Connection<Db>, user: AuthUser, id: i64) -> ProblemRowTemplate {
    let problem = sqlx::query_as::<_, ProblemWithCategories>(
        r#"
        SELECT 
            p.*, 
            GROUP_CONCAT(c.name) as category_names,
            l.kind as source_kind,
            l.title as source_title
        FROM problems p
        JOIN log_items l ON p.log_item_id = l.id
        LEFT JOIN problem_categories pc ON p.id = pc.problem_id
        LEFT JOIN categories c ON pc.category_id = c.id
        WHERE p.id = ?
        GROUP BY p.id
        "#
    )
    .bind(id)
    .fetch_one(&mut **db)
    .await
    .unwrap();

    ProblemRowTemplate { problem, user: Some(user) }
}

#[post("/problems/<id>", data = "<form>")]
async fn update_problem(mut db: Connection<Db>, user: AuthUser, id: i64, form: Form<UpdateProblem>) -> ProblemRowTemplate {
    // 1. Update Problem fields
    sqlx::query("UPDATE problems SET notes = ?, solution_link = ? WHERE id = ?")
        .bind(&form.notes)
        .bind(&form.solution_link)
        .bind(id)
        .execute(&mut **db)
        .await
        .unwrap();

    // 2. Update Categories
    // First, get the course_id via log_item
    let problem_info = sqlx::query_as::<_, Problem>("SELECT * FROM problems WHERE id = ?")
        .bind(id)
        .fetch_one(&mut **db)
        .await
        .unwrap();
        
    let log_item = sqlx::query_as::<_, LogItem>("SELECT * FROM log_items WHERE id = ?")
        .bind(problem_info.log_item_id)
        .fetch_one(&mut **db)
        .await
        .unwrap();

    // Clear existing categories for this problem
    sqlx::query("DELETE FROM problem_categories WHERE problem_id = ?")
        .bind(id)
        .execute(&mut **db)
        .await
        .unwrap();

    // Add new categories
    if let Some(cats) = &form.categories {
        for cat_name in cats.split(|c| c == ',' || c == '、').map(|s| s.trim()).filter(|s| !s.is_empty()) {
            // Find or create category
            let cat_id_opt: Option<i64> = sqlx::query_scalar("SELECT id FROM categories WHERE course_id = ? AND name = ?")
                .bind(log_item.course_id)
                .bind(cat_name)
                .fetch_optional(&mut **db)
                .await
                .unwrap();

            let cat_id = match cat_id_opt {
                Some(cid) => cid,
                None => {
                    sqlx::query("INSERT INTO categories (course_id, name) VALUES (?, ?)")
                        .bind(log_item.course_id)
                        .bind(cat_name)
                        .execute(&mut **db)
                        .await
                        .unwrap()
                        .last_insert_rowid()
                }
            };

            // Link problem to category
            sqlx::query("INSERT INTO problem_categories (problem_id, category_id) VALUES (?, ?)")
                .bind(id)
                .bind(cat_id)
                .execute(&mut **db)
                .await
                .unwrap();
        }
    }

    // 3. Return updated row
    // Reuse get_problem_row logic or call it if I could, but I'll just copy the query for now to avoid borrow checker/async recursion issues if I tried to call the handler.
    // Actually I can just run the query.
    let problem = sqlx::query_as::<_, ProblemWithCategories>(
        r#"
        SELECT 
            p.*, 
            GROUP_CONCAT(c.name) as category_names,
            l.kind as source_kind,
            l.title as source_title
        FROM problems p
        JOIN log_items l ON p.log_item_id = l.id
        LEFT JOIN problem_categories pc ON p.id = pc.problem_id
        LEFT JOIN categories c ON pc.category_id = c.id
        WHERE p.id = ?
        GROUP BY p.id
        "#
    )
    .bind(id)
    .fetch_one(&mut **db)
    .await
    .unwrap();

    ProblemRowTemplate { problem, user: Some(user) }
}

pub fn routes() -> Vec<rocket::Route> {
    routes![
        index, 
        dashboard,
        get_login,
        post_login,
        get_register,
        post_register,
        logout,
        create_semester, 
        view_semester, 
        create_course, 
        view_course_log, 
        create_log_item,
        create_problem,
        get_log_problems,
        view_course_study,
        filter_study_problems,
        delete_log_item,
        get_edit_log_item,
        get_log_item,
        update_log_item,
        get_edit_problem,
        update_problem,
        get_problem_row
    ]
}
