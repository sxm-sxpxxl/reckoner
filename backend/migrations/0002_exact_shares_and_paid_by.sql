-- Точные суммы вместо четвертей и «платит за» внутри встречи.
-- Дизайн: docs/superpowers/specs/2026-10-03-exact-split-design.md.

-- Неполные четверти (¼, ½, ¾) в новую модель напрямую не ложатся, и перед
-- деплоем проверено, что их нет ни в одной базе. Если всё-таки появились,
-- миграция падает целиком: сервер не поднимется, и Render оставит работать
-- прежнюю версию. Это лучше, чем молча поменять чьи-то деньги.
do $$
begin
    if exists (select 1 from entry_shares where weight_quarters between 1 and 3) then
        raise exception
            'в entry_shares есть неполные доли (¼, ½, ¾): их нужно перевести в рубли вручную, см. docs/superpowers/specs/2026-10-03-exact-split-design.md';
    end if;
end
$$;

-- Строки нет — участник делит остаток поровну. 0 — исключён из расхода.
-- Больше нуля — точная сумма. После проверки выше остались только нулевые
-- доли, то есть исключённые, и значение по умолчанию переводит их ровно в 0.
alter table entry_shares add column rubles bigint not null default 0 check (rubles >= 0);
alter table entry_shares alter column rubles drop default;
alter table entry_shares drop column weight_quarters;

-- Кто платит за участника. Принадлежность той же встрече и отсутствие цепочек
-- проверяет API — так же, как для payer_id и recipient_id у записей.
alter table participants
    add column paid_by uuid references participants (id) on delete set null,
    add constraint participants_paid_by_not_self check (paid_by <> id);
