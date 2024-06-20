-- Add migration script here
-- Add a table update notification function
CREATE OR REPLACE FUNCTION table_update_notify() RETURNS trigger AS $$
DECLARE
  id varchar;
  name varchar;
BEGIN
  IF TG_OP = 'INSERT' OR TG_OP = 'UPDATE' THEN
    id = NEW.id;
    name = NEW.name;
  ELSE
    id = OLD.id;
    name = old.name;
  END IF;
  PERFORM pg_notify('table_update', json_build_object('table', TG_TABLE_NAME, 'id', id, 'name', name, 'action_type', TG_OP)::text);
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Add UPDATE row trigger
CREATE OR REPLACE TRIGGER tasks_notify_update AFTER UPDATE ON tasks FOR EACH ROW EXECUTE PROCEDURE table_update_notify();

-- Add INSERT row trigger
CREATE OR REPLACE TRIGGER tasks_notify_insert AFTER INSERT ON tasks FOR EACH ROW EXECUTE PROCEDURE table_update_notify();

-- Add DELETE row trigger
CREATE OR REPLACE TRIGGER tasks_notify_delete AFTER DELETE ON tasks FOR EACH ROW EXECUTE PROCEDURE table_update_notify();