# PLAIn teXT

Plaixt is a set of conventions and modules to interact with data stored in
plain text. It is purpose geared towards storing and interacting with 'real
things'. Aka, concrete actions and/or objects.
It might not do well with abstract concepts like 'infinity' or mutually
recursive events.
As a rule of thumb, if it has a date, and possibly a duration.
It will probably fit.

## What do I use this for?

Plaixt is made to interact with plain text files. It's primary use case is to
record data and then process it further.

This is purposefully kept vague, just like a Hammer is a hard heavy object to
hit things with.

Some things that you can do with Plaixt:

- Record purchases in your home, making sure that each purchase has an
  associated receipt, price and warranty expiration date.
- Record contacts, personal or professional.
- Link between entries, making sure that the links are of correct kinds

## What is the data format?

Plaixt uses a plain text, humand readable and writeable format.

A plaixt repository consists of these parts:

- A definitions folder
- A records folder


### Definitions

To make sense of the records, plaixt needs to know how records look like.
This is so that both tools and humans know what to expect and how to interact
with it.

- Each definition is in a separate file.
    - The filename is the 'kind' of the record, and may not have whitespaces in it
    - An optional `.pldef` may be appended to make recognition of these files easier
- Definitions are counted in epochs, which is the date from which this
  definition is considered live.
- As definitions may evolve with time, new versions of a definition may be added.
    - These are appended at the end of the file with a date, making the
      previous definition now obselete and making this definition the new live
      one.
    - Records _before_ this date, per default only have to conform to
      definitions that were live at the time of event. This can be overriden
      individually.
    - 'Migrating' to a newer definition can be done piece-by-piece, or all at
      once by changing the starting date from which a definition is considered
      live.
    - Order of precedence of definition is in reverse declaration order,
      followed by starting live date order. (e.g. a definition declared later
      in a file with the same starting live date will take precedence over
      earlier definitions with the same or later starting live date)

### Records

A record is anything one would want to track.
Every record must have an associated datetime.
Every record may have an associated unique id.
Beyond these requirements, each record must have a kind associated with it.
The definition of this kind then describes the required data of the record.


## Syntax

This is still in flux, but the main idea is as following:

### Definitions

An example definition:

**purchase.pldef**
```plaixt
# This is a comment
// You can also use double slashes
/* Multi-line comments are ok too */

# Definitions may use modules to verify records
# They can for example check that records are valid between eachother across
# the whole database
@checkWith "purchase-check"

# Each new definition needs a date from which it is active
# A date suffices, which implies a starting time of 00:00
define 26-10-2024T11:00
    store -> LinkTo[Store]
    name -> string
    warranty length -> duration? # A question mark, marks optional fields, a shorthand for Maybe[Field]
    count -> integer

# I realized I forgot the price field
# Any records before this date won't have a price field
define 15-11-2024
    store -> LinkTo[Store]
    name -> string
    warranty length -> duration?
    count -> integer
    price -> euros
```

### Records

An example record:

**shopping.plrecs**
```plaixt
purchase 30-10-2024
    store <- FarmerBernard
    name <- "Pumpkin"
    count <- 5
    # No price here, as the record at this time does not allow adding a price

# I can force plaixt to use the definition from this specific date
purchase@15-11-2024 5-11-2024
    store <- DIYCo
    name <- "Nails"
    count <- 250
    price <- 3.50 # I have to mention the price, as the definition
                  # from 15-11-2024 has it as an non-optional field
```


## Interacting with plaixt

Once you've defined some records, and wrote down some records, you can now
query your database.

For example, imagine we want to know what items we own that are no longer under
warranty.

```bash
plaixt --query "SELECT * FROM purchase WHERE 'warranty length' NOT NULL AND date + 'warranty length' < now()"
```

Of course this is a rather crude way of interacting with the system, and
instead one would use helpers:

```bash
plaixt purchases out-of-warranty
```

These helpers are modules that have access to your database and encode the
utility of it all.
This readme only has a simple example, but imagine a slightly more complete
inventory tracking which has acquiring _and_ removing of inventory.
The question of 'what I do I have at this point in time' comes out of the data,
but is not written into it.


## Why plain text? Couldn't I use sqlite/postgres/mongo/toml/etc... ?

Technically you can.

Plaixt is modular and does not _require_ its input to come
from text files on a drive.
But let me convince you why plain text is the better format to use:

- It will survive any of these database technologies. It's Unicode in files.
  Both of which are concepts that are in my opinion way less likely to simply
  disappear.
- Humans can easily interact with it. Everyone should be able to write plaixt
  records without having to learn it. Simply by looking at the other
  definitions it should be clear what is going on and how to enter new text.
- Any current and future technology is able to interact with it. You don't want
  to learn Rust? No problem. It's plain text! Write your own logic and helpers
  in whatever language you want to. If you can read/write bytes from
  stdin/stdout and/or files, you are set. For databases you will need to have
  drivers, complex connection handling etc... All of which is not required.
  (We're also not stopping you though)
- Plain text has the big advantage to also be future compatible. You enter
  records now, and can analyze them later. Due to the flexibility of plain
  text, you do not need to first change the software to adapt.

### What about non-plain text data? I have PDFs and images, and they are part of records

We do take that into account!
While plaixt per-default does not have any special handling for binary files,
it doesn't preclude you from integrating them into your database.
For example, the example above could be adapted to require a receipt in
pdf/image form for each purchase.
This can take many forms.
The simplest would be to only require a file path, relative to the datastore.
But integration with other software is also possible.
Imagine a module that knows how to speak to a paperless instance.
One could simply enter the ID of the document there, and the module only checks
for existence.
The possibilities here are unbounded, and allow you to custom fit your
interactions with your needs.

# Contributing

This project is currently incubating, and as such there is not much to
contribute to.
Nontheless, if you feel like this project speaks to you specifically, and you
want to participate, you can always open an issue or talk to me on other
platforms.

# License

The plaixt code is licensed under the EUPL-1.2 or later.
Any other documents, not covered by the EUPL-1.2 or later licenses, are licensed under CC-BY-SA 4.0.

To make it simple, do what you want privately, but if you share it with others,
or allow them to use it, make sure that they know of their rights.

-------

<p xmlns:cc="http://creativecommons.org/ns#" xmlns:dct="http://purl.org/dc/terms/"><span property="dct:title">Plaixt</span> by <span property="cc:attributionName">Marcel Müller</span> is licensed under <a href="https://creativecommons.org/licenses/by-sa/4.0/?ref=chooser-v1" target="_blank" rel="license noopener noreferrer" style="display:inline-block;">CC BY-SA 4.0<img style="height:22px!important;margin-left:3px;vertical-align:text-bottom;" src="https://mirrors.creativecommons.org/presskit/icons/cc.svg?ref=chooser-v1" alt=""><img style="height:22px!important;margin-left:3px;vertical-align:text-bottom;" src="https://mirrors.creativecommons.org/presskit/icons/by.svg?ref=chooser-v1" alt=""><img style="height:22px!important;margin-left:3px;vertical-align:text-bottom;" src="https://mirrors.creativecommons.org/presskit/icons/sa.svg?ref=chooser-v1" alt=""></a></p>
