# Contributing to catena

Thanks for helping. Two things are required before any contribution can merge: every commit is
signed off, and by signing off you agree to the contributor license grant below. The rest of
this file is how the work is organised.

## Sign-off and license grant (required)

`catena` is licensed AGPL-3.0-only, and its owner also sells commercial exceptions to that
license. Selling an exception requires the right to relicense every line in the repository, so
each contribution carries both the Developer Certificate of Origin and the grant in this
section. A pull request whose commits are not signed off cannot be merged, and CI checks this on
pull requests from forks.

Sign off every commit with `git commit -s`, which appends a trailer with your real name and
email address:

```text
Signed-off-by: Your Name <you@example.com>
```

That line certifies two things: the Developer Certificate of Origin 1.1 below, and your
agreement to the Contributor License Grant below, for the contribution in that commit.

### Developer Certificate of Origin 1.1

```text
Developer Certificate of Origin
Version 1.1

Copyright (C) 2004, 2006 The Linux Foundation and its contributors.

Everyone is permitted to copy and distribute verbatim copies of this
license document, but changing it is not allowed.


Developer's Certificate of Origin 1.1

By making a contribution to this project, I certify that:

(a) The contribution was created in whole or in part by me and I
    have the right to submit it under the open source license
    indicated in the file; or

(b) The contribution is based upon previous work that, to the best
    of my knowledge, is covered under an appropriate open source
    license and I have the right under that license to submit that
    work with modifications, whether created in whole or in part
    by me, under the same open source license (unless I am
    permitted to submit under a different license), as indicated
    in the file; or

(c) The contribution was provided directly to me by some other
    person who certified (a), (b) or (c) and I have not modified
    it.

(d) I understand and agree that this project and the contribution
    are public and that a record of the contribution (including all
    personal information I submit with it, including my sign-off) is
    maintained indefinitely and may be redistributed consistent with
    this project or the open source license(s) involved.
```

### Contributor License Grant

In this grant, "you" means the person signing off; "Owner" means the holder of the GitHub
account `jbrjake`, who maintains `github.com/jbrjake/catena`, and the Owner's successors and
assigns; "Contribution" means any work of authorship you submit to this repository, including
modifications of existing work.

1. **Copyright license.** You grant the Owner a perpetual, worldwide, non-exclusive, no-charge,
   royalty-free, irrevocable license to reproduce, prepare derivative works of, publicly
   display, publicly perform, sublicense and distribute your Contribution and such derivative
   works, under any license terms the Owner chooses. Those terms include AGPL-3.0-only and
   proprietary or commercial licenses, such as the commercial exceptions the Owner sells.
2. **Patent license.** You grant the Owner, and every recipient of software the Owner
   distributes, a perpetual, worldwide, non-exclusive, no-charge, royalty-free, irrevocable
   patent license to make, have made, use, offer to sell, sell, import and otherwise transfer
   your Contribution. It covers only the patent claims you can license that your Contribution
   necessarily infringes, alone or combined with the work it was submitted to.
3. **Your rights.** You keep the copyright in your Contribution. Everyone else receives it under
   AGPL-3.0-only, the same license as the rest of the repository.
4. **Your representations.** The Contribution is your original work, or you have the right to
   submit it under this grant. If an employer or anyone else holds rights in it, they have
   permitted the submission under these terms. You will say so in the pull request if any part
   of the Contribution is not your own work, naming its source and license.
5. **No other obligations.** The Owner need not use your Contribution. Apart from the
   representations above, you provide it "as is", without warranties of any kind.

## Working on the code

- `docs/design/initial-catena-plan.md` is the design authority, and its §21 is ruled. A
  departure from it is a question for the owner, raised as an issue before the code.
- `TODO.md` is the worklist. Its `## Now` names the next step, and every item closes on a
  `verify:` command observed to exit 0.
- `seed/` is harvest input: never compiled and never edited in place. A file leaves it by the
  two-commit procedure in plan §18.
- Commits follow Conventional Commits (`type(scope): subject`), are atomic, and update the docs
  they affect in the same commit.
