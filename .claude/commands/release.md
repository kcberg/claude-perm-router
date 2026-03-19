Cut a new release of claude-perm-router.

Steps:
1. Make sure you're on the main branch and it's up to date: `git checkout main && git pull`
2. Check the current version in Cargo.toml and ask the user what kind of bump they want (patch, minor, or major)
3. Run `cargo set-version --bump <level>` to bump the version
4. Run `cargo generate-lockfile` to update Cargo.lock
5. Create a release branch: `git checkout -b release/v<new_version>`
6. Commit: `git add Cargo.toml Cargo.lock && git commit -m "chore: bump version to <new_version>"`
7. Push the branch: `git push -u origin release/v<new_version>`
8. Create a PR with title "Release v<new_version>" using `gh pr create`
9. Tell the user: "PR created. After CI passes and you merge it, run these commands to trigger the release build:"
10. Print the post-merge commands:
    ```
    git checkout main && git pull
    git tag v<new_version>
    git push origin v<new_version>
    ```
