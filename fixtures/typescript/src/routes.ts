import { Request, Response } from "express";
import { getPersonById, newPerson, PersonData } from "./service.js";

type PersonRequest = Request<{}, {}, { name: string; email: string }>;
type PersonResponse = Response<PersonData | { error: string }>;

export enum ResponseStatus {
  SUCCESS = 200,
  CREATED = 201,
  NOT_FOUND = 404,
  INTERNAL_ERROR = 500,
}
export function registerRoutes(app) {
  app.get("/person/:id", getPerson);

  app.post("/person", createPerson);
}

async function getPerson(req: Request, res: Response) {
  const { id } = req.params;

  try {
    const person = (await getPersonById(Number(id))) as PersonData;
    if (!person) {
      return res
        .status(ResponseStatus.NOT_FOUND)
        .json({ error: "Person not found" });
    }
    return res.json(person);
  } catch (error) {
    console.error(error);
    return res
      .status(ResponseStatus.INTERNAL_ERROR)
      .json({ error: "Internal server error" });
  }
}

async function createPerson(req: PersonRequest, res: PersonResponse) {
  const { name, email } = req.body;
  try {
    const person: PersonData = await newPerson({ name, email });
    return res.status(ResponseStatus.CREATED).json(person);
  } catch (error) {
    console.error(error);
    return res
      .status(ResponseStatus.INTERNAL_ERROR)
      .json({ error: "Internal server error" });
  }
}
