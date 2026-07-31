CREATE UNIQUE INDEX IF NOT EXISTS print_jobs_retry_source
ON print_jobs(retry_of_job_id)
WHERE retry_of_job_id IS NOT NULL;
