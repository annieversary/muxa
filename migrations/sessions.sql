-- Add migration script here
CREATE TABLE IF NOT EXISTS sessions (
  `id` VARCHAR(128) NOT NULL,
  `expires` DATETIME NOT NULL,
  `session` TEXT NOT NULL,
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
