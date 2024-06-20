-- Add migration script here
-- Add a table update notification function
CREATE OR REPLACE FUNCTION table_update_notify() RETURNS trigger AS $$
DECLARE
  rec RECORD;
  dat RECORD;
BEGIN
  CASE TG_OP
  WHEN 'UPDATE' THEN
     rec := NEW;
     dat := OLD;
  WHEN 'INSERT' THEN
     rec := NEW;
  WHEN 'DELETE' THEN
     rec := OLD;
  ELSE
     RAISE EXCEPTION 'Unknown TG_OP: "%". Should not occur!', TG_OP;
  END CASE;

  PERFORM pg_notify(
    'table_update',
    json_build_object(
      'timestamp', CURRENT_TIMESTAMP,
      'action', UPPER(TG_OP),
      'table', TG_TABLE_NAME,
      'id', rec.id,
      -- Include the entire new and old rows.
      -- This doesn't scale though, there's a 3KB size limit on notify
      'record', row_to_json(rec)::text,
      'old', row_to_json(dat)::text
    )::text
  );

  RETURN rec;
END;
$$ LANGUAGE plpgsql;

-- Add UPDATE row trigger
CREATE OR REPLACE TRIGGER tasks_notify_update AFTER UPDATE ON tasks FOR EACH ROW EXECUTE PROCEDURE table_update_notify();

-- Add INSERT row trigger
CREATE OR REPLACE TRIGGER tasks_notify_insert AFTER INSERT ON tasks FOR EACH ROW EXECUTE PROCEDURE table_update_notify();

-- Add DELETE row trigger
CREATE OR REPLACE TRIGGER tasks_notify_delete AFTER DELETE ON tasks FOR EACH ROW EXECUTE PROCEDURE table_update_notify();