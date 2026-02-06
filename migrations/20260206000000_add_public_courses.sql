ALTER TABLE courses ADD COLUMN is_published INTEGER NOT NULL DEFAULT 0;
ALTER TABLE courses ADD COLUMN public_slug TEXT;
ALTER TABLE courses ADD COLUMN show_lecture_links INTEGER NOT NULL DEFAULT 0;

CREATE UNIQUE INDEX idx_courses_public_slug ON courses(public_slug) WHERE public_slug IS NOT NULL;

CREATE TABLE translations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_text TEXT NOT NULL,
    translated_text TEXT NOT NULL,
    source_lang TEXT NOT NULL DEFAULT 'zh',
    target_lang TEXT NOT NULL DEFAULT 'en',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX idx_translations_source ON translations(source_text, source_lang, target_lang);
