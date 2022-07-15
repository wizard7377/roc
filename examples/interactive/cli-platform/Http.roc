interface Http
    exposes [
        header,
        emptyBody,
        bytesBody,
        stringBody,
        jsonBody,
        multiPartBody,
        stringPart,
        bytesPart,
        handleStringResponse,
        defaultRequest,
        errorToString,
        send,
    ]
    imports [
        Effect,
        InternalTask,
        Task,
        Encode.{ Encoding },
        HttpTypes.{ Request, Header, TimeoutConfig, TrackerConfig, Part, Body, Response, Metadata, Error },
    ]

defaultRequest : Request
defaultRequest = {
    method: "GET",
    headers: [],
    url: "",
    body: Http.emptyBody,
    timeout: NoTimeout,
    tracker: NoTracker,
    allowCookiesFromOtherDomains: False,
}

## An HTTP header for configuring requests. See a bunch of common headers
## [here](https://en.wikipedia.org/wiki/List_of_HTTP_header_fields).
##
header : Str, Str -> Header
header = \name, value ->
    { name, value }

emptyBody : Body
emptyBody =
    EmptyBody

bytesBody : [MimeType Str], List U8 -> Body
bytesBody =
    Body

stringBody : [MimeType Str], Str -> Body
stringBody = \mimeType, str ->
    Body mimeType (Str.toUtf8 str)

jsonBody : a -> Body | a has Encoding
jsonBody = \val ->
    Body (MimeType "application/json") (Encode.toBytes val Json.format)

multiPartBody : List Part -> Body
multiPartBody = \parts ->
    boundary = "7MA4YWxkTrZu0gW" # TODO: what's this exactly? a hash of all the part bodies?
    beforeName = Str.toUtf8 "-- \(boundary)\r\nContent-Disposition: form-data; name=\""
    afterName = Str.toUtf8 "\"\r\n"
    appendPart = \buffer, Part name partBytes ->
        buffer
            |> List.concat beforeName
            |> List.concat (Str.toUtf8 name)
            |> List.concat afterName
            |> List.concat partBytes
    bodyBytes = List.walk parts [] appendPart

    Body (MimeType "multipart/form-data;boundary=\"\(boundary)\"") bodyBytes

bytesPart : Str, List U8 -> Part
bytesPart =
    Part

stringPart : Str, Str -> Part
stringPart = \name, str ->
    Part name (Str.toUtf8 str)

handleStringResponse : Response -> Result Str Error
handleStringResponse = \response ->
    when response is
        BadUrl url -> Err (BadUrl url)
        Timeout -> Err Timeout
        NetworkError -> Err NetworkError
        BadStatus metadata _ -> Err (BadStatus metadata.statusCode)
        GoodStatus _ bodyBytes ->
            Str.fromUtf8 bodyBytes
                |> Result.mapErr
                    \BadUtf8 _ pos ->
                        position = Num.toStr pos

                        BadBody "Invalid UTF-8 at byte offset \(position)"

errorToString : Error -> Str
errorToString = \err ->
    when err is
        BadUrl url -> "\(url) is not a valid URL"
        Timeout -> "Request timed out"
        NetworkError -> "Network error"
        BadStatus code -> Str.concat "Request failed with status " (Num.toStr code)
        BadBody details -> Str.concat "Request failed. Invalid body. " details

send : Request -> Task Str Error [Network [Http]*]*
send = \req ->
    Effect.sendRequest req
        |> Effect.map handleStringResponse
        |> InternalTask.fromEffect
