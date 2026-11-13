# Autolinks

These examples run with the `autolink` extension enabled.
The first examples are the specification's own; the rest pin the
reference's boundary behavior. Every expected output agrees with the
reference implementation and with the engine, byte for byte.


```````````````````````````````` example
www.commonmark.org
.
<p><a href="http://www.commonmark.org">www.commonmark.org</a></p>
````````````````````````````````

```````````````````````````````` example
Visit www.commonmark.org/help for more information.
.
<p>Visit <a href="http://www.commonmark.org/help">www.commonmark.org/help</a> for more information.</p>
````````````````````````````````

```````````````````````````````` example
Visit www.commonmark.org.

Visit www.commonmark.org/a.b.
.
<p>Visit <a href="http://www.commonmark.org">www.commonmark.org</a>.</p>
<p>Visit <a href="http://www.commonmark.org/a.b">www.commonmark.org/a.b</a>.</p>
````````````````````````````````

```````````````````````````````` example
www.google.com/search?q=Markup+(business)

www.google.com/search?q=Markup+(business)))

(www.google.com/search?q=Markup+(business))

(www.google.com/search?q=Markup+(business)
.
<p><a href="http://www.google.com/search?q=Markup+(business)">www.google.com/search?q=Markup+(business)</a></p>
<p><a href="http://www.google.com/search?q=Markup+(business)">www.google.com/search?q=Markup+(business)</a>))</p>
<p>(<a href="http://www.google.com/search?q=Markup+(business)">www.google.com/search?q=Markup+(business)</a>)</p>
<p>(<a href="http://www.google.com/search?q=Markup+(business)">www.google.com/search?q=Markup+(business)</a></p>
````````````````````````````````

```````````````````````````````` example
www.google.com/search?q=(business))+ok
.
<p><a href="http://www.google.com/search?q=(business))+ok">www.google.com/search?q=(business))+ok</a></p>
````````````````````````````````

```````````````````````````````` example
www.google.com/search?q=commonmark&hl=en

www.google.com/search?q=commonmark&hl;
.
<p><a href="http://www.google.com/search?q=commonmark&amp;hl=en">www.google.com/search?q=commonmark&amp;hl=en</a></p>
<p><a href="http://www.google.com/search?q=commonmark">www.google.com/search?q=commonmark</a>&amp;hl;</p>
````````````````````````````````

```````````````````````````````` example
www.commonmark.org/he<lp
.
<p><a href="http://www.commonmark.org/he">www.commonmark.org/he</a>&lt;lp</p>
````````````````````````````````

```````````````````````````````` example
http://commonmark.org

(Visit https://encrypted.google.com/search?q=Markup+(business))

Anonymous FTP is available at ftp://foo.bar.baz.
.
<p><a href="http://commonmark.org">http://commonmark.org</a></p>
<p>(Visit <a href="https://encrypted.google.com/search?q=Markup+(business)">https://encrypted.google.com/search?q=Markup+(business)</a>)</p>
<p>Anonymous FTP is available at <a href="ftp://foo.bar.baz">ftp://foo.bar.baz</a>.</p>
````````````````````````````````

```````````````````````````````` example
foo@bar.baz
.
<p><a href="mailto:foo@bar.baz">foo@bar.baz</a></p>
````````````````````````````````

```````````````````````````````` example
hello@mail+xyz.example isn't valid, but hello+xyz@mail.example is.
.
<p>hello@mail+xyz.example isn't valid, but <a href="mailto:hello+xyz@mail.example">hello+xyz@mail.example</a> is.</p>
````````````````````````````````

```````````````````````````````` example
a.b-c_d@a.b

a.b-c_d@a.b.

a.b-c_d@a.b-

a.b-c_d@a.b_
.
<p><a href="mailto:a.b-c_d@a.b">a.b-c_d@a.b</a></p>
<p><a href="mailto:a.b-c_d@a.b">a.b-c_d@a.b</a>.</p>
<p>a.b-c_d@a.b-</p>
<p>a.b-c_d@a.b_</p>
````````````````````````````````

```````````````````````````````` example
www.example.com
.
<p><a href="http://www.example.com">www.example.com</a></p>
````````````````````````````````

```````````````````````````````` example
www.commonmark.org/help<br/>a
.
<p><a href="http://www.commonmark.org/help">www.commonmark.org/help</a><br/>a</p>
````````````````````````````````

```````````````````````````````` example
Visit www.commonmark.org/help for more information
.
<p>Visit <a href="http://www.commonmark.org/help">www.commonmark.org/help</a> for more information</p>
````````````````````````````````

```````````````````````````````` example
www.google.com/search?q=Mark+down&ua=Zulu&body=hello+world&email=abc%40def.gh
.
<p><a href="http://www.google.com/search?q=Mark+down&amp;ua=Zulu&amp;body=hello+world&amp;email=abc%40def.gh">www.google.com/search?q=Mark+down&amp;ua=Zulu&amp;body=hello+world&amp;email=abc%40def.gh</a></p>
````````````````````````````````

```````````````````````````````` example
www.commonmark.org/he<lp
.
<p><a href="http://www.commonmark.org/he">www.commonmark.org/he</a>&lt;lp</p>
````````````````````````````````

```````````````````````````````` example
http://commonmark.org?gssd
.
<p><a href="http://commonmark.org?gssd">http://commonmark.org?gssd</a></p>
````````````````````````````````

```````````````````````````````` example
(test) www.google.com(search)q
.
<p>(test) <a href="http://www.google.com(search)q">www.google.com(search)q</a></p>
````````````````````````````````

```````````````````````````````` example
http://www.google.com
.
<p><a href="http://www.google.com">http://www.google.com</a></p>
````````````````````````````````

```````````````````````````````` example
ftp://ftp.example.com/x
.
<p><a href="ftp://ftp.example.com/x">ftp://ftp.example.com/x</a></p>
````````````````````````````````

```````````````````````````````` example
https://EXAMPLE.com
.
<p><a href="https://EXAMPLE.com">https://EXAMPLE.com</a></p>
````````````````````````````````

```````````````````````````````` example
HTTP://EXAMPLE.COM
.
<p><a href="HTTP://EXAMPLE.COM">HTTP://EXAMPLE.COM</a></p>
````````````````````````````````

```````````````````````````````` example
not:a-url
.
<p>not:a-url</p>
````````````````````````````````

```````````````````````````````` example
x:not:a-url
.
<p>x:not:a-url</p>
````````````````````````````````

```````````````````````````````` example
ftp:/x
.
<p>ftp:/x</p>
````````````````````````````````

```````````````````````````````` example
//www.example.com
.
<p>//www.example.com</p>
````````````````````````````````

```````````````````````````````` example
www.benchmark.xyz
.
<p><a href="http://www.benchmark.xyz">www.benchmark.xyz</a></p>
````````````````````````````````

```````````````````````````````` example
a@b.com
.
<p><a href="mailto:a@b.com">a@b.com</a></p>
````````````````````````````````

```````````````````````````````` example
test@example.com
.
<p><a href="mailto:test@example.com">test@example.com</a></p>
````````````````````````````````

```````````````````````````````` example
x@y.z
.
<p><a href="mailto:x@y.z">x@y.z</a></p>
````````````````````````````````

```````````````````````````````` example
foo@bar.example.com
.
<p><a href="mailto:foo@bar.example.com">foo@bar.example.com</a></p>
````````````````````````````````

```````````````````````````````` example
foo@bar_1.com
.
<p><a href="mailto:foo@bar_1.com">foo@bar_1.com</a></p>
````````````````````````````````

```````````````````````````````` example
foo@bar_baz.com
.
<p><a href="mailto:foo@bar_baz.com">foo@bar_baz.com</a></p>
````````````````````````````````

```````````````````````````````` example
mail me at foo@bar.baz now
.
<p>mail me at <a href="mailto:foo@bar.baz">foo@bar.baz</a> now</p>
````````````````````````````````

```````````````````````````````` example
foo@bar.baz.
.
<p><a href="mailto:foo@bar.baz">foo@bar.baz</a>.</p>
````````````````````````````````

```````````````````````````````` example
(foo@bar.com)
.
<p>(<a href="mailto:foo@bar.com">foo@bar.com</a>)</p>
````````````````````````````````

```````````````````````````````` example
foo@bar.com.vn.
.
<p><a href="mailto:foo@bar.com.vn">foo@bar.com.vn</a>.</p>
````````````````````````````````

```````````````````````````````` example
"hi, this is a link"foo@a.com
.
<p>&quot;hi, this is a link&quot;<a href="mailto:foo@a.com">foo@a.com</a></p>
````````````````````````````````

```````````````````````````````` example
mailto:foo@bar.baz
.
<p><a href="mailto:foo@bar.baz">mailto:foo@bar.baz</a></p>
````````````````````````````````

```````````````````````````````` example
XMPP:foo@bar.example.com
.
<p>XMPP:<a href="mailto:foo@bar.example.com">foo@bar.example.com</a></p>
````````````````````````````````

```````````````````````````````` example
xmpp:foo@bar.example.com
.
<p><a href="xmpp:foo@bar.example.com">xmpp:foo@bar.example.com</a></p>
````````````````````````````````

```````````````````````````````` example
mailto:foo@bar.baz.example
.
<p><a href="mailto:foo@bar.baz.example">mailto:foo@bar.baz.example</a></p>
````````````````````````````````

```````````````````````````````` example
 xmpp:foo@bar.example.com/x
.
<p><a href="xmpp:foo@bar.example.com/x">xmpp:foo@bar.example.com/x</a></p>
````````````````````````````````

```````````````````````````````` example
xmpp:foo@bar.example.com/x/y
.
<p><a href="xmpp:foo@bar.example.com/x/y">xmpp:foo@bar.example.com/x/y</a></p>
````````````````````````````````

```````````````````````````````` example
link: http://example.com
.
<p>link: <a href="http://example.com">http://example.com</a></p>
````````````````````````````````

```````````````````````````````` example
www.google.com. www.google.com,
.
<p><a href="http://www.google.com">www.google.com</a>. <a href="http://www.google.com">www.google.com</a>,</p>
````````````````````````````````

```````````````````````````````` example
www.google.com/search?q=Mark+down&ua=Zulu&body=hello+world&email=abc%40def.gh,
.
<p><a href="http://www.google.com/search?q=Mark+down&amp;ua=Zulu&amp;body=hello+world&amp;email=abc%40def.gh">www.google.com/search?q=Mark+down&amp;ua=Zulu&amp;body=hello+world&amp;email=abc%40def.gh</a>,</p>
````````````````````````````````

```````````````````````````````` example
(www.google.com/search?q=Mark+down&ua=Zulu)
.
<p>(<a href="http://www.google.com/search?q=Mark+down&amp;ua=Zulu">www.google.com/search?q=Mark+down&amp;ua=Zulu</a>)</p>
````````````````````````````````

```````````````````````````````` example
http://www.google.com/search?q=Mark+down&ua=Zulu&body=hello+world&email=abc%40def.gh
.
<p><a href="http://www.google.com/search?q=Mark+down&amp;ua=Zulu&amp;body=hello+world&amp;email=abc%40def.gh">http://www.google.com/search?q=Mark+down&amp;ua=Zulu&amp;body=hello+world&amp;email=abc%40def.gh</a></p>
````````````````````````````````

```````````````````````````````` example
Invalid url www.xinvalid url
.
<p>Invalid url <a href="http://www.xinvalid">www.xinvalid</a> url</p>
````````````````````````````````

```````````````````````````````` example
www.commonmark.org/he<lp
.
<p><a href="http://www.commonmark.org/he">www.commonmark.org/he</a>&lt;lp</p>
````````````````````````````````

```````````````````````````````` example
www.commonmark.org(a+b)
.
<p><a href="http://www.commonmark.org(a+b)">www.commonmark.org(a+b)</a></p>
````````````````````````````````

```````````````````````````````` example
www.google.com/search?q=Mark+down&ua=Zulu&body=hello+world&email=abc%40def.gh#fragment
.
<p><a href="http://www.google.com/search?q=Mark+down&amp;ua=Zulu&amp;body=hello+world&amp;email=abc%40def.gh#fragment">www.google.com/search?q=Mark+down&amp;ua=Zulu&amp;body=hello+world&amp;email=abc%40def.gh#fragment</a></p>
````````````````````````````````

```````````````````````````````` example
-http://example.com-
.
<p>-<a href="http://example.com-">http://example.com-</a></p>
````````````````````````````````

```````````````````````````````` example
link foo@bar.baz.google.com
.
<p>link <a href="mailto:foo@bar.baz.google.com">foo@bar.baz.google.com</a></p>
````````````````````````````````

```````````````````````````````` example
foo@bar.baz.google.com.
.
<p><a href="mailto:foo@bar.baz.google.com">foo@bar.baz.google.com</a>.</p>
````````````````````````````````

```````````````````````````````` example
e@mail.cc
.
<p><a href="mailto:e@mail.cc">e@mail.cc</a></p>
````````````````````````````````

```````````````````````````````` example
a@b.c
.
<p><a href="mailto:a@b.c">a@b.c</a></p>
````````````````````````````````

```````````````````````````````` example
foo@-bar-.com
.
<p><a href="mailto:foo@-bar-.com">foo@-bar-.com</a></p>
````````````````````````````````

```````````````````````````````` example
foo@bar..com
.
<p>foo@bar..com</p>
````````````````````````````````

```````````````````````````````` example
foo@bar.com-
.
<p>foo@bar.com-</p>
````````````````````````````````

```````````````````````````````` example
foo@bar.com-
.
<p>foo@bar.com-</p>
````````````````````````````````

```````````````````````````````` example
a@1.2.3.4.5.6.7.8.9.10.11.12
.
<p>a@1.2.3.4.5.6.7.8.9.10.11.12</p>
````````````````````````````````

```````````````````````````````` example
foo@a_b.com
.
<p><a href="mailto:foo@a_b.com">foo@a_b.com</a></p>
````````````````````````````````

```````````````````````````````` example
foo@a__.b.com
.
<p><a href="mailto:foo@a__.b.com">foo@a__.b.com</a></p>
````````````````````````````````

```````````````````````````````` example
foo@a_b.example.com
.
<p><a href="mailto:foo@a_b.example.com">foo@a_b.example.com</a></p>
````````````````````````````````

```````````````````````````````` example
foo@a_.b.example.com
.
<p><a href="mailto:foo@a_.b.example.com">foo@a_.b.example.com</a></p>
````````````````````````````````

```````````````````````````````` example
foo@a._.b.example.com
.
<p>foo@a._.b.example.com</p>
````````````````````````````````

```````````````````````````````` example
www.a_b.com
.
<p>www.a_b.com</p>
````````````````````````````````

```````````````````````````````` example
www.a_b.example.com
.
<p><a href="http://www.a_b.example.com">www.a_b.example.com</a></p>
````````````````````````````````

```````````````````````````````` example
www.a_.b.example.com
.
<p><a href="http://www.a_.b.example.com">www.a_.b.example.com</a></p>
````````````````````````````````

```````````````````````````````` example
www.a._.b.example.com
.
<p><a href="http://www.a._.b.example.com">www.a._.b.example.com</a></p>
````````````````````````````````

```````````````````````````````` example
foo@111.222.333.44444444
.
<p>foo@111.222.333.44444444</p>
````````````````````````````````

```````````````````````````````` example
*hi www.google.com*
.
<p><em>hi <a href="http://www.google.com">www.google.com</a></em></p>
````````````````````````````````

```````````````````````````````` example
_foo_bar@example.com
.
<p><a href="mailto:_foo_bar@example.com">_foo_bar@example.com</a></p>
````````````````````````````````

```````````````````````````````` example
foo@example.com_
.
<p>foo@example.com_</p>
````````````````````````````````

```````````````````````````````` example
**foo@bar.com**
.
<p><strong><a href="mailto:foo@bar.com">foo@bar.com</a></strong></p>
````````````````````````````````

```````````````````````````````` example
*foo@bar.com
.
<p>*<a href="mailto:foo@bar.com">foo@bar.com</a></p>
````````````````````````````````

```````````````````````````````` example
foo@bar.com*
.
<p><a href="mailto:foo@bar.com">foo@bar.com</a>*</p>
````````````````````````````````

```````````````````````````````` example
foo@bar.baz    xxx
.
<p><a href="mailto:foo@bar.baz">foo@bar.baz</a>    xxx</p>
````````````````````````````````

```````````````````````````````` example
mailto:foo@bar.baz.example.com
.
<p><a href="mailto:foo@bar.baz.example.com">mailto:foo@bar.baz.example.com</a></p>
````````````````````````````````

```````````````````````````````` example
xmp:+foo@bar.example.com
.
<p>xmp:<a href="mailto:+foo@bar.example.com">+foo@bar.example.com</a></p>
````````````````````````````````

```````````````````````````````` example
xmpp:foo@bar.example.com/
.
<p>xmpp:foo@bar.example.com/</p>
````````````````````````````````

```````````````````````````````` example
xmpp:foo@bar.example.com//x
.
<p><a href="xmpp:foo@bar.example.com//x">xmpp:foo@bar.example.com//x</a></p>
````````````````````````````````

```````````````````````````````` example
xmpp:foo@bar.example.com://
.
<p><a href="xmpp:foo@bar.example.com">xmpp:foo@bar.example.com</a>://</p>
````````````````````````````````

```````````````````````````````` example
mail foo@bar.baz.example.com more
.
<p>mail <a href="mailto:foo@bar.baz.example.com">foo@bar.baz.example.com</a> more</p>
````````````````````````````````

```````````````````````````````` example
-tbl_user@dev.example.com
.
<p><a href="mailto:-tbl_user@dev.example.com">-tbl_user@dev.example.com</a></p>
````````````````````````````````

```````````````````````````````` example
mailto:-tbl_user@dev.example.com
.
<p><a href="mailto:-tbl_user@dev.example.com">mailto:-tbl_user@dev.example.com</a></p>
````````````````````````````````

```````````````````````````````` example
xmpp:-tbl_user@dev.example.com
.
<p><a href="xmpp:-tbl_user@dev.example.com">xmpp:-tbl_user@dev.example.com</a></p>
````````````````````````````````
