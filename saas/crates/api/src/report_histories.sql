WITH amounts AS (
 SELECT h.id,h.code,h.description AS label,h.active,
 coalesce(sum(e.amount_cents::numeric) FILTER(WHERE e.direction=1),0) AS income,
 coalesce(sum(e.amount_cents::numeric) FILTER(WHERE e.direction=-1),0) AS expense
 FROM histories h LEFT JOIN entries e ON e.history_id=h.id AND e.tenant_id=h.tenant_id AND NOT e.cancelled AND e.entry_date BETWEEN $1::text::date AND $2::text::date
 GROUP BY h.id
)
SELECT code,label,active,income::text,expense::text,(income-expense)::text AS net FROM amounts ORDER BY code,label
