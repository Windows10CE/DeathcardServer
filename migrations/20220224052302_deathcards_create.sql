CREATE TABLE IF NOT EXISTS users (
    user_id SERIAL PRIMARY KEY,
    secret_key int8 NOT NULL
);

CREATE TABLE IF NOT EXISTS deathcards (
    owner_id SERIAL REFERENCES users ON DELETE CASCADE,
    card_id SERIAL PRIMARY KEY,
    card_name text NOT NULL,
    attack integer NOT NULL CHECK (attack >= 0),
    health integer NOT NULL CHECK (health > 0),
    abilities text[] NOT NULL,
    special_abilities text[] NOT NULL,
    stat_icon text NOT NULL,
    blood_cost int NOT NULL CHECK (bone_cost >= 0),
    bone_cost int NOT NULL CHECK (bone_cost >= 0),
    gem_cost int[] NOT NULL
);
