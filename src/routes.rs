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

#[derive(Template)]
#[template(path = "course_exams.html")]
struct CourseExamsTemplate {
    course: Course,
    courses: Vec<Course>,
    exams: Vec<Exam>,
    semester: Semester,
    categories: Vec<Category>,
    user: Option<AuthUser>,
}

#[derive(Template)]
#[template(path = "partials/exam_item.html")]
struct ExamItemTemplate {
    exam: Exam,
    categories: Vec<Category>,
    user: Option<AuthUser>,
}

#[derive(Template)]
#[template(path = "partials/exam_item_edit.html")]
struct ExamItemEditTemplate {
    exam: Exam,
    user: Option<AuthUser>,
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
    is_incorrect: Option<String>,
}

#[derive(FromForm)]
struct UpdateProblem {
    notes: Option<String>,
    solution_link: Option<String>,
    categories: Option<String>,
    is_incorrect: Option<String>,
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

#[derive(FromForm)]
struct NewExam {
    title: String,
    date: Option<String>,
}

#[derive(FromForm)]
struct UpdateExam {
    title: String,
    date: Option<String>,
}

// Shared query for fetching a problem with categories
const PROBLEM_WITH_CATEGORIES_QUERY: &str = r#"
    SELECT
        p.id, p.log_item_id, p.exam_id, p.description, p.notes, p.image_url, p.solution_link, p.is_incorrect,
        GROUP_CONCAT(c.name) as category_names,
        COALESCE(l.kind, 'Exam') as source_kind,
        COALESCE(l.title, e.title, '') as source_title
    FROM problems p
    LEFT JOIN log_items l ON p.log_item_id = l.id
    LEFT JOIN exams e ON p.exam_id = e.id
    LEFT JOIN problem_categories pc ON p.id = pc.problem_id
    LEFT JOIN categories c ON pc.category_id = c.id
    WHERE p.id = ?
    GROUP BY p.id
"#;

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
    if user.is_none() {
         return Redirect::to("/login");
    }
    Redirect::to("/dashboard")
}

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
        created_at: String::new(),
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
    let problems = sqlx::query("SELECT id FROM problems WHERE log_item_id = ?")
        .bind(id)
        .fetch_all(&mut **db)
        .await
        .unwrap_or_default();

    for problem in problems {
        let problem_id: i64 = problem.try_get("id").unwrap();
        sqlx::query("DELETE FROM problem_categories WHERE problem_id = ?")
            .bind(problem_id)
            .execute(&mut **db)
            .await
            .unwrap();
    }

    sqlx::query("DELETE FROM problems WHERE log_item_id = ?")
        .bind(id)
        .execute(&mut **db)
        .await
        .unwrap();

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
    let file_name = format!("{}.png", Uuid::new_v4());
    let file_path = format!("uploads/{}", file_name);
    form.screenshot.move_copy_to(&file_path).await.expect("Unable to move or copy file");
    let image_url = format!("/uploads/{}", file_name);

    let description = "Screenshot Problem";
    let is_incorrect: bool = form.is_incorrect.as_deref() == Some("on");

    let problem_id = sqlx::query("INSERT INTO problems (log_item_id, description, notes, image_url, solution_link, is_incorrect) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(id)
        .bind(description)
        .bind(&form.notes)
        .bind(&image_url)
        .bind(&form.solution_link)
        .bind(is_incorrect)
        .execute(&mut **db)
        .await
        .unwrap()
        .last_insert_rowid();

    let mut category_names = String::new();
    if let Some(cats) = &form.categories {
        let log_item = sqlx::query_as::<_, LogItem>("SELECT * FROM log_items WHERE id = ?")
            .bind(id)
            .fetch_one(&mut **db)
            .await
            .unwrap();

        let mut processed_cats = Vec::new();
        for cat_name in cats.split(|c| c == ',' || c == '\u{3001}').map(|s| s.trim()).filter(|s| !s.is_empty()) {
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

    let problem = ProblemWithCategories {
        id: problem_id,
        log_item_id: Some(id),
        exam_id: None,
        description: description.to_string(),
        notes: form.notes.clone(),
        image_url: Some(image_url),
        solution_link: form.solution_link.clone(),
        is_incorrect,
        category_names: if category_names.is_empty() { None } else { Some(category_names) },
        source_kind: "".to_string(),
        source_title: "".to_string(),
    };

    ProblemRowTemplate { problem, user: Some(user) }
}

#[get("/logs/<id>/problems")]
async fn get_log_problems(mut db: Connection<Db>, _user: AuthUser, id: i64) -> String {
    let problems = sqlx::query_as::<_, ProblemWithCategories>(
        r#"
        SELECT
            p.id, p.log_item_id, p.exam_id, p.description, p.notes, p.image_url, p.solution_link, p.is_incorrect,
            GROUP_CONCAT(c.name) as category_names,
            COALESCE(l.kind, 'Exam') as source_kind,
            COALESCE(l.title, e.title, '') as source_title
        FROM problems p
        LEFT JOIN log_items l ON p.log_item_id = l.id
        LEFT JOIN exams e ON p.exam_id = e.id
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

    let mut html = String::new();
    for p in problems {
        let t = ProblemRowTemplate { problem: p, user: None };
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

#[get("/courses/<id>/study/problems?<source>&<category>&<incorrect>")]
async fn filter_study_problems(mut db: Connection<Db>, _user: AuthUser, id: i64, source: Option<Vec<String>>, category: Option<Vec<String>>, incorrect: Option<String>) -> StudyProblemListTemplate {
    let mut query = String::from(
        r#"
        SELECT
            p.id, p.log_item_id, p.exam_id, p.description, p.notes, p.image_url, p.solution_link, p.is_incorrect,
            GROUP_CONCAT(c.name) as category_names,
            COALESCE(l.kind, 'Exam') as source_kind,
            COALESCE(l.title, e.title, '') as source_title
        FROM problems p
        LEFT JOIN log_items l ON p.log_item_id = l.id
        LEFT JOIN exams e ON p.exam_id = e.id
        LEFT JOIN problem_categories pc ON p.id = pc.problem_id
        LEFT JOIN categories c ON pc.category_id = c.id
        WHERE (l.course_id = ? OR e.course_id = ?)
        "#
    );

    // Filter by incorrect only
    if incorrect.as_deref() == Some("on") {
        query.push_str(" AND p.is_incorrect = 1");
    }

    // Filter by Source
    if let Some(sources) = &source {
        if !sources.is_empty() {
            let has_exam = sources.iter().any(|s| s == "Exam");
            let log_sources: Vec<&String> = sources.iter().filter(|s| *s != "Exam").collect();

            if has_exam && !log_sources.is_empty() {
                query.push_str(" AND (l.kind IN (");
                for (i, s) in log_sources.iter().enumerate() {
                    if i > 0 { query.push_str(", "); }
                    query.push_str(&format!("'{}'", s));
                }
                query.push_str(") OR p.exam_id IS NOT NULL)");
            } else if has_exam {
                query.push_str(" AND p.exam_id IS NOT NULL");
            } else {
                query.push_str(" AND l.kind IN (");
                for (i, s) in log_sources.iter().enumerate() {
                    if i > 0 { query.push_str(", "); }
                    query.push_str(&format!("'{}'", s));
                }
                query.push_str(")");
            }
        }
    }

    // Filter by Category
    if let Some(cats) = &category {
         if !cats.is_empty() {
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
        .bind(id)
        .fetch_all(&mut **db)
        .await
        .unwrap_or_default();

    StudyProblemListTemplate { problems, user: None }
}

#[get("/problems/<id>/edit")]
async fn get_edit_problem(mut db: Connection<Db>, user: AuthUser, id: i64) -> ProblemEditTemplate {
    let problem = sqlx::query_as::<_, ProblemWithCategories>(PROBLEM_WITH_CATEGORIES_QUERY)
        .bind(id)
        .fetch_one(&mut **db)
        .await
        .unwrap();

    ProblemEditTemplate { problem, user: Some(user) }
}

#[get("/problems/<id>")]
async fn get_problem_row(mut db: Connection<Db>, user: AuthUser, id: i64) -> ProblemRowTemplate {
    let problem = sqlx::query_as::<_, ProblemWithCategories>(PROBLEM_WITH_CATEGORIES_QUERY)
        .bind(id)
        .fetch_one(&mut **db)
        .await
        .unwrap();

    ProblemRowTemplate { problem, user: Some(user) }
}

#[post("/problems/<id>", data = "<form>")]
async fn update_problem(mut db: Connection<Db>, user: AuthUser, id: i64, form: Form<UpdateProblem>) -> ProblemRowTemplate {
    let is_incorrect: bool = form.is_incorrect.as_deref() == Some("on");

    sqlx::query("UPDATE problems SET notes = ?, solution_link = ?, is_incorrect = ? WHERE id = ?")
        .bind(&form.notes)
        .bind(&form.solution_link)
        .bind(is_incorrect)
        .bind(id)
        .execute(&mut **db)
        .await
        .unwrap();

    // Get the course_id via log_item or exam
    let problem_info = sqlx::query_as::<_, Problem>("SELECT * FROM problems WHERE id = ?")
        .bind(id)
        .fetch_one(&mut **db)
        .await
        .unwrap();

    let course_id: i64 = if let Some(log_item_id) = problem_info.log_item_id {
        let log_item = sqlx::query_as::<_, LogItem>("SELECT * FROM log_items WHERE id = ?")
            .bind(log_item_id)
            .fetch_one(&mut **db)
            .await
            .unwrap();
        log_item.course_id
    } else if let Some(exam_id) = problem_info.exam_id {
        let exam = sqlx::query_as::<_, Exam>("SELECT * FROM exams WHERE id = ?")
            .bind(exam_id)
            .fetch_one(&mut **db)
            .await
            .unwrap();
        exam.course_id
    } else {
        panic!("Problem has neither log_item_id nor exam_id");
    };

    // Clear existing categories for this problem
    sqlx::query("DELETE FROM problem_categories WHERE problem_id = ?")
        .bind(id)
        .execute(&mut **db)
        .await
        .unwrap();

    // Add new categories
    if let Some(cats) = &form.categories {
        for cat_name in cats.split(|c| c == ',' || c == '\u{3001}').map(|s| s.trim()).filter(|s| !s.is_empty()) {
            let cat_id_opt: Option<i64> = sqlx::query_scalar("SELECT id FROM categories WHERE course_id = ? AND name = ?")
                .bind(course_id)
                .bind(cat_name)
                .fetch_optional(&mut **db)
                .await
                .unwrap();

            let cat_id = match cat_id_opt {
                Some(cid) => cid,
                None => {
                    sqlx::query("INSERT INTO categories (course_id, name) VALUES (?, ?)")
                        .bind(course_id)
                        .bind(cat_name)
                        .execute(&mut **db)
                        .await
                        .unwrap()
                        .last_insert_rowid()
                }
            };

            sqlx::query("INSERT INTO problem_categories (problem_id, category_id) VALUES (?, ?)")
                .bind(id)
                .bind(cat_id)
                .execute(&mut **db)
                .await
                .unwrap();
        }
    }

    let problem = sqlx::query_as::<_, ProblemWithCategories>(PROBLEM_WITH_CATEGORIES_QUERY)
        .bind(id)
        .fetch_one(&mut **db)
        .await
        .unwrap();

    ProblemRowTemplate { problem, user: Some(user) }
}

// ========== Exam Routes ==========

#[get("/courses/<id>/exams")]
async fn view_course_exams(mut db: Connection<Db>, user: AuthUser, id: i64) -> CourseExamsTemplate {
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

    let exams = sqlx::query_as::<_, Exam>("SELECT * FROM exams WHERE course_id = ? ORDER BY date DESC, id DESC")
        .bind(id)
        .fetch_all(&mut **db)
        .await
        .unwrap_or_default();

    let categories = sqlx::query_as::<_, Category>("SELECT * FROM categories WHERE course_id = ?")
        .bind(id)
        .fetch_all(&mut **db)
        .await
        .unwrap_or_default();

    CourseExamsTemplate { course, courses, exams, semester, categories, user: Some(user) }
}

#[post("/courses/<id>/exams", data = "<form>")]
async fn create_exam(mut db: Connection<Db>, user: AuthUser, id: i64, form: Form<NewExam>) -> ExamItemTemplate {
    let exam_id = sqlx::query("INSERT INTO exams (course_id, title, date) VALUES (?, ?, ?)")
        .bind(id)
        .bind(&form.title)
        .bind(&form.date)
        .execute(&mut **db)
        .await
        .unwrap()
        .last_insert_rowid();

    let exam = Exam {
        id: exam_id,
        course_id: id,
        title: form.title.clone(),
        date: form.date.clone(),
    };

    let categories = sqlx::query_as::<_, Category>("SELECT * FROM categories WHERE course_id = ?")
        .bind(id)
        .fetch_all(&mut **db)
        .await
        .unwrap_or_default();

    ExamItemTemplate { exam, categories, user: Some(user) }
}

#[get("/exams/<id>")]
async fn get_exam(mut db: Connection<Db>, user: AuthUser, id: i64) -> ExamItemTemplate {
    let exam = sqlx::query_as::<_, Exam>("SELECT * FROM exams WHERE id = ?")
        .bind(id)
        .fetch_one(&mut **db)
        .await
        .unwrap();

    let categories = sqlx::query_as::<_, Category>("SELECT * FROM categories WHERE course_id = ?")
        .bind(exam.course_id)
        .fetch_all(&mut **db)
        .await
        .unwrap_or_default();

    ExamItemTemplate { exam, categories, user: Some(user) }
}

#[get("/exams/<id>/edit")]
async fn get_edit_exam(mut db: Connection<Db>, user: AuthUser, id: i64) -> ExamItemEditTemplate {
    let exam = sqlx::query_as::<_, Exam>("SELECT * FROM exams WHERE id = ?")
        .bind(id)
        .fetch_one(&mut **db)
        .await
        .unwrap();
    ExamItemEditTemplate { exam, user: Some(user) }
}

#[post("/exams/<id>", data = "<form>")]
async fn update_exam(mut db: Connection<Db>, user: AuthUser, id: i64, form: Form<UpdateExam>) -> ExamItemTemplate {
    sqlx::query("UPDATE exams SET title = ?, date = ? WHERE id = ?")
        .bind(&form.title)
        .bind(&form.date)
        .bind(id)
        .execute(&mut **db)
        .await
        .unwrap();

    let exam = sqlx::query_as::<_, Exam>("SELECT * FROM exams WHERE id = ?")
        .bind(id)
        .fetch_one(&mut **db)
        .await
        .unwrap();

    let categories = sqlx::query_as::<_, Category>("SELECT * FROM categories WHERE course_id = ?")
        .bind(exam.course_id)
        .fetch_all(&mut **db)
        .await
        .unwrap_or_default();

    ExamItemTemplate { exam, categories, user: Some(user) }
}

#[delete("/exams/<id>")]
async fn delete_exam(mut db: Connection<Db>, _user: AuthUser, id: i64) -> String {
    // Cascade delete: problem_categories -> problems -> exam
    let problems = sqlx::query("SELECT id FROM problems WHERE exam_id = ?")
        .bind(id)
        .fetch_all(&mut **db)
        .await
        .unwrap_or_default();

    for problem in problems {
        let problem_id: i64 = problem.try_get("id").unwrap();
        sqlx::query("DELETE FROM problem_categories WHERE problem_id = ?")
            .bind(problem_id)
            .execute(&mut **db)
            .await
            .unwrap();
    }

    sqlx::query("DELETE FROM problems WHERE exam_id = ?")
        .bind(id)
        .execute(&mut **db)
        .await
        .unwrap();

    sqlx::query("DELETE FROM exams WHERE id = ?")
        .bind(id)
        .execute(&mut **db)
        .await
        .unwrap();

    String::new()
}

#[post("/exams/<id>/problems", data = "<form>")]
async fn create_exam_problem(mut db: Connection<Db>, user: AuthUser, id: i64, mut form: Form<NewProblem<'_>>) -> ProblemRowTemplate {
    let file_name = format!("{}.png", Uuid::new_v4());
    let file_path = format!("uploads/{}", file_name);
    form.screenshot.move_copy_to(&file_path).await.expect("Unable to move or copy file");
    let image_url = format!("/uploads/{}", file_name);

    let description = "Screenshot Problem";
    let is_incorrect: bool = form.is_incorrect.as_deref() == Some("on");

    let problem_id = sqlx::query("INSERT INTO problems (exam_id, description, notes, image_url, solution_link, is_incorrect) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(id)
        .bind(description)
        .bind(&form.notes)
        .bind(&image_url)
        .bind(&form.solution_link)
        .bind(is_incorrect)
        .execute(&mut **db)
        .await
        .unwrap()
        .last_insert_rowid();

    let mut category_names = String::new();
    if let Some(cats) = &form.categories {
        let exam = sqlx::query_as::<_, Exam>("SELECT * FROM exams WHERE id = ?")
            .bind(id)
            .fetch_one(&mut **db)
            .await
            .unwrap();

        let mut processed_cats = Vec::new();
        for cat_name in cats.split(|c| c == ',' || c == '\u{3001}').map(|s| s.trim()).filter(|s| !s.is_empty()) {
            let cat_id_opt: Option<i64> = sqlx::query_scalar("SELECT id FROM categories WHERE course_id = ? AND name = ?")
                .bind(exam.course_id)
                .bind(cat_name)
                .fetch_optional(&mut **db)
                .await
                .unwrap();

            let cat_id = match cat_id_opt {
                Some(cid) => cid,
                None => {
                    sqlx::query("INSERT INTO categories (course_id, name) VALUES (?, ?)")
                        .bind(exam.course_id)
                        .bind(cat_name)
                        .execute(&mut **db)
                        .await
                        .unwrap()
                        .last_insert_rowid()
                }
            };

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

    let problem = ProblemWithCategories {
        id: problem_id,
        log_item_id: None,
        exam_id: Some(id),
        description: description.to_string(),
        notes: form.notes.clone(),
        image_url: Some(image_url),
        solution_link: form.solution_link.clone(),
        is_incorrect,
        category_names: if category_names.is_empty() { None } else { Some(category_names) },
        source_kind: "Exam".to_string(),
        source_title: "".to_string(),
    };

    ProblemRowTemplate { problem, user: Some(user) }
}

#[get("/exams/<id>/problems")]
async fn get_exam_problems(mut db: Connection<Db>, _user: AuthUser, id: i64) -> String {
    let problems = sqlx::query_as::<_, ProblemWithCategories>(
        r#"
        SELECT
            p.id, p.log_item_id, p.exam_id, p.description, p.notes, p.image_url, p.solution_link, p.is_incorrect,
            GROUP_CONCAT(c.name) as category_names,
            COALESCE(l.kind, 'Exam') as source_kind,
            COALESCE(l.title, e.title, '') as source_title
        FROM problems p
        LEFT JOIN log_items l ON p.log_item_id = l.id
        LEFT JOIN exams e ON p.exam_id = e.id
        LEFT JOIN problem_categories pc ON p.id = pc.problem_id
        LEFT JOIN categories c ON pc.category_id = c.id
        WHERE p.exam_id = ?
        GROUP BY p.id
        "#
    )
    .bind(id)
    .fetch_all(&mut **db)
    .await
    .unwrap_or_default();

    let mut html = String::new();
    for p in problems {
        let t = ProblemRowTemplate { problem: p, user: None };
        html.push_str(&t.render().unwrap());
    }
    html
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
        get_problem_row,
        view_course_exams,
        create_exam,
        get_exam,
        get_edit_exam,
        update_exam,
        delete_exam,
        create_exam_problem,
        get_exam_problems
    ]
}
