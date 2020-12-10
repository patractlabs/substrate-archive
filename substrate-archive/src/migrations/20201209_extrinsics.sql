CREATE TABLE IF NOT EXISTS extrinsics (
  id SERIAL  PRIMARY KEY,
  hash bytea NOT NULL ,
   block_num int check (block_num >= 0 and block_num < 2147483647) NOT NULL ,
    index int  NOT NULL ,
    address bytea ,
    signature bytea ,
    extra bytea ,
    fun bytea NOT NULL,
    UNIQUE (block_num,index)
);
