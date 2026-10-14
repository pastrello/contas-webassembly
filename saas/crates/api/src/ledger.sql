WITH totals AS (
 SELECT a.opening_balance_cents,
 coalesce(sum(e.amount_cents::numeric*e.direction) FILTER(WHERE NOT e.cancelled),0)::text AS movement,
 coalesce(sum(e.amount_cents::numeric*e.direction) FILTER(WHERE NOT e.cancelled AND e.cleared),0)::text AS bank,
 coalesce(sum(e.amount_cents::numeric*e.direction) FILTER(WHERE NOT e.cancelled AND e.entry_date<$2::text::date),0)::text AS previous,
 coalesce(sum(e.amount_cents::numeric) FILTER(WHERE NOT e.cancelled AND e.entry_date BETWEEN $2::text::date AND $3::text::date AND e.direction=1),0)::text AS income,
 coalesce(sum(e.amount_cents::numeric) FILTER(WHERE NOT e.cancelled AND e.entry_date BETWEEN $2::text::date AND $3::text::date AND e.direction=-1),0)::text AS expense,
 count(e.id) FILTER(WHERE e.entry_date BETWEEN $2::text::date AND $3::text::date AND ($4 OR NOT e.archived) AND ($5 OR NOT e.cancelled)) AS total
 FROM accounts a LEFT JOIN entries e ON e.account_id=a.id AND e.tenant_id=a.tenant_id
 WHERE a.id=$1 GROUP BY a.id
), page AS (
 SELECT e.id,e.account_id,e.history_id,to_char(e.entry_date,'YYYY-MM-DD') AS date,
 e.document,e.description,(e.amount_cents::numeric/100)::numeric(20,2)::text AS amount,
 e.direction,e.cleared,e.archived,e.cancelled,e.version,h.code||' · '||h.description AS history_label,e.sequence
 FROM entries e JOIN histories h ON h.id=e.history_id AND h.tenant_id=e.tenant_id
 WHERE e.account_id=$1 AND e.entry_date BETWEEN $2::text::date AND $3::text::date
 AND ($4 OR NOT e.archived) AND ($5 OR NOT e.cancelled)
 ORDER BY e.entry_date,e.sequence LIMIT 200 OFFSET $6
)
SELECT totals.*,coalesce((SELECT jsonb_agg(to_jsonb(p)-'sequence' ORDER BY p.date,p.sequence) FROM page p),'[]'::jsonb) AS items FROM totals
