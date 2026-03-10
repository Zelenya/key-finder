create table if not exists apps (
  id integer primary key autoincrement,
  name text not null unique,
  canonical_name text not null
);

create table if not exists shortcuts (
  id integer primary key autoincrement,
  app_id integer not null references apps(id) on delete cascade,
  shortcut_display text not null,
  shortcut_norm text not null,
  description text not null,
  state text not null default 'active',
  created_at integer not null,
  updated_at integer not null,
  unique(app_id, shortcut_norm, description)
);

create index if not exists idx_shortcuts_app_created on shortcuts(app_id, state, created_at);

create table if not exists imports (
  id integer primary key autoincrement,
  app_id integer references apps(id) on delete set null,
  started_at integer not null,
  finished_at integer,
  status text not null,
  error_text text
);

create table if not exists app_aliases (
  id integer primary key autoincrement,
  app_id integer not null references apps(id) on delete cascade,
  alias text not null,
  canonical_alias text not null,
  unique(app_id, canonical_alias)
);

create index if not exists idx_app_aliases_app_alias on app_aliases(app_id, alias collate nocase);
create index if not exists idx_app_aliases_app_canonical_alias on app_aliases(app_id, canonical_alias);

create table if not exists settings (
  key text primary key,
  value text not null,
  updated_at integer not null
);
