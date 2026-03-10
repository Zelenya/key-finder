create table if not exists app_importers (
  app_id integer primary key references apps(id) on delete cascade,
  importer_family text not null
);
