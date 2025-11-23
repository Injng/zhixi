CREATE TABLE semesters (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE courses (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    semester_id INTEGER NOT NULL,
    code TEXT NOT NULL,
    title TEXT NOT NULL,
    FOREIGN KEY (semester_id) REFERENCES semesters(id)
);

CREATE TABLE log_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    course_id INTEGER NOT NULL,
    kind TEXT NOT NULL, -- 'Lecture', 'Lab', 'Discussion', 'Homework', 'Midterm', 'Quiz', 'Other'
    title TEXT NOT NULL,
    description TEXT,
    link TEXT,
    date TEXT, -- ISO8601 date string
    FOREIGN KEY (course_id) REFERENCES courses(id)
);

CREATE TABLE categories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    course_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    FOREIGN KEY (course_id) REFERENCES courses(id)
);

CREATE TABLE problems (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    log_item_id INTEGER NOT NULL,
    description TEXT NOT NULL,
    notes TEXT,
    image_url TEXT,
    FOREIGN KEY (log_item_id) REFERENCES log_items(id)
);

CREATE TABLE problem_categories (
    problem_id INTEGER NOT NULL,
    category_id INTEGER NOT NULL,
    PRIMARY KEY (problem_id, category_id),
    FOREIGN KEY (problem_id) REFERENCES problems(id),
    FOREIGN KEY (category_id) REFERENCES categories(id)
);
