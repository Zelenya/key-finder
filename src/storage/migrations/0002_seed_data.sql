insert or ignore into settings(key, value, updated_at)
values('cooldown', '10m', strftime('%s', 'now'));

insert or ignore into settings(key, value, updated_at)
values('app_switch_bounce', '30s', strftime('%s', 'now'));

insert or ignore into settings(key, value, updated_at)
values('shortcut_focus_count', '5', strftime('%s', 'now'));

insert or ignore into apps(id, name, canonical_name)
values
  (1, 'IntelliJ IDEA', 'intellijidea'),
  (2, 'Visual Studio Code', 'visualstudiocode'),
  (3, 'Zed', 'zed');

insert or ignore into app_aliases(app_id, alias, canonical_alias)
values
  (1, 'PyCharm', 'pycharm'),
  (1, 'WebStorm', 'webstorm'),
  (1, 'Android Studio', 'androidstudio');

insert or ignore into app_aliases(app_id, alias, canonical_alias)
values
  (2, 'Code', 'code');

insert or ignore into app_aliases(app_id, alias, canonical_alias)
values
  (3, 'Zed Preview', 'zedpreview');
