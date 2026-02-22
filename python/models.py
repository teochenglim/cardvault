from typing import Optional
from pydantic import BaseModel


class Phone(BaseModel):
    id: int = 0
    label: str = "mobile"
    number: str


class Email(BaseModel):
    id: int = 0
    label: str = "work"
    address: str


class Address(BaseModel):
    id: int = 0
    label: str = "office"
    street: str = ""
    city: str = ""
    country: str = ""
    postal: str = ""


class Card(BaseModel):
    id: int = 0
    name: str
    title: str = ""
    company: str = ""
    website: str = ""
    notes: str = ""
    photo_url: str = ""
    phones: list[Phone] = []
    emails: list[Email] = []
    addresses: list[Address] = []
    tags: list[str] = []
    created_at: str = ""
    updated_at: str = ""


class TagCount(BaseModel):
    name: str
    count: int


class HealthResponse(BaseModel):
    status: str
    db: str


class PhoneInput(BaseModel):
    label: str = "mobile"
    number: str


class EmailInput(BaseModel):
    label: str = "work"
    address: str


class AddressInput(BaseModel):
    label: str = "office"
    street: str = ""
    city: str = ""
    country: str = ""
    postal: str = ""
