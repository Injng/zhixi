-- Create exams table
CREATE TABLE exams (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    course_id INTEGER NOT NULL,
    title TEXT NOT NULL,
    date TEXT,
    FOREIGN KEY (course_id) REFERENCES courses(id)
);

-- Recreate problems table with nullable log_item_id, new exam_id, and is_incorrect
-- Must also drop/recreate problem_categories to avoid FK constraint on DROP TABLE problems
CREATE TABLE problems_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    log_item_id INTEGER,
    exam_id INTEGER,
    description TEXT NOT NULL,
    notes TEXT,
    image_url TEXT,
    solution_link TEXT,
    is_incorrect INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (log_item_id) REFERENCES log_items(id),
    FOREIGN KEY (exam_id) REFERENCES exams(id)
);

-- Copy existing problem data
INSERT INTO problems_new (id, log_item_id, description, notes, image_url, solution_link)
    SELECT id, log_item_id, description, notes, image_url, solution_link FROM problems;

-- Save problem_categories data, drop it, then drop old problems
CREATE TABLE problem_categories_backup (
    problem_id INTEGER NOT NULL,
    category_id INTEGER NOT NULL
);
INSERT INTO problem_categories_backup SELECT problem_id, category_id FROM problem_categories;

DROP TABLE problem_categories;
DROP TABLE problems;
ALTER TABLE problems_new RENAME TO problems;

-- Recreate problem_categories referencing the new problems table
CREATE TABLE problem_categories (
    problem_id INTEGER NOT NULL,
    category_id INTEGER NOT NULL,
    PRIMARY KEY (problem_id, category_id),
    FOREIGN KEY (problem_id) REFERENCES problems(id),
    FOREIGN KEY (category_id) REFERENCES categories(id)
);
INSERT INTO problem_categories SELECT problem_id, category_id FROM problem_categories_backup;
DROP TABLE problem_categories_backup;
