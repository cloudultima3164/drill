create table if not exists example_table 
(
    id              bigserial not null,
    code            numeric(20),
    repr_name       varchar(128),
    color           varchar(50),
    primary key (id, code, repr_name)
);

