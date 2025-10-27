import { Sequelize } from "sequelize";
import { DataSource } from "typeorm";
import { PrismaClient } from "@prisma/client";
import { TypeORMPerson } from "./model.js";

export const sequelize = new Sequelize({
  dialect: "sqlite",
  storage: "./database.sqlite",
  logging: false,
});

export const AppDataSource = new DataSource({
  type: "sqlite",
  database: "./database.sqlite",
  entities: [TypeORMPerson],
  synchronize: true,
  logging: false,
});

export const prisma = new PrismaClient();
