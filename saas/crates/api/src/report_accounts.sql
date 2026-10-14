WITH amounts AS (
 SELECT a.id,a.code,a.name AS label,a.active,a.opening_balance_cents::numeric AS initial,
 coalesce(sum(e.amount_cents::numeric*e.direction) FILTER(WHERE e.entry_date<$1::text::date),0) AS before,
 coalesce(sum(e.amount_cents::numeric) FILTER(WHERE e.entry_date BETWEEN $1::text::date AND $2::text::date AND e.direction=1),0) AS income,
 coalesce(sum(e.amount_cents::numeric) FILTER(WHERE e.entry_date BETWEEN $1::text::date AND $2::text::date AND e.direction=-1),0) AS expense,
 coalesce(sum(e.amount_cents::numeric*e.direction) FILTER(WHERE e.cleared AND e.entry_date<=$2::text::date),0) AS cleared
 FROM accounts a LEFT JOIN entries e ON e.account_id=a.id AND e.tenant_id=a.tenant_id AND NOT e.cancelled
 GROUP BY a.id
)
SELECT code,label,active,initial::text,(initial+before)::text AS previous,income::text,expense::text,(initial+before+income-expense)::text AS closing,(initial+cleared)::text AS bank FROM amounts ORDER BY code,label
