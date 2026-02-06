-- Create exams table
CREATE TABLE exams (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    course_id INTEGER NOT NULL,
    title TEXT NOT NULL,
    date TEXT,
    FOREIGN KEY (course_id) REFERENCES courses(id)
);

-- Recreate problems table with nullable log_item_id, new exam_id, and is_incorrect
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

-- Copy existing data
INSERT INTO problems_new (id, log_item_id, description, notes, image_url, solution_link)
    SELECT id, log_item_id, description, notes, image_url, solution_link FROM problems;

-- Drop old table and rename
DROP TABLE problems;
ALTER TABLE problems_new RENAME TO problems;

-- Recreate problem_categories (foreign key references are fine since table name is the same)
-- The junction table references problems(id) which still exists with same IDs
